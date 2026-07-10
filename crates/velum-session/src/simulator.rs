//! Deterministic in-memory carrier fault model.
//!
//! The simulator owns only carrier scheduling. It never acknowledges or
//! delivers data on behalf of [`crate::SessionTracer`].

use std::collections::VecDeque;

use crate::Segment;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CarrierDisposition {
    Deliver,
    Drop,
    Duplicate,
    Delay(u64),
}

#[derive(Clone, Debug)]
struct ScheduledSegment {
    ready_at: u64,
    segment: Segment,
}

/// A deterministic carrier model for loss, duplication, reordering, delay,
/// black holes, and recovery tests.
#[derive(Debug, Default)]
pub struct InMemoryCarrier {
    tick: u64,
    available: bool,
    scheduled: VecDeque<ScheduledSegment>,
}

impl InMemoryCarrier {
    pub fn new() -> Self {
        Self {
            available: true,
            ..Self::default()
        }
    }

    /// Switches the carrier into or out of a black-hole state.
    pub fn set_available(&mut self, available: bool) {
        self.available = available;
    }

    /// Schedules one carrier attempt. Attempts made during a black hole drop.
    pub fn transmit(&mut self, segment: Segment, disposition: CarrierDisposition) {
        if !self.available || disposition == CarrierDisposition::Drop {
            return;
        }

        let delay = match disposition {
            CarrierDisposition::Delay(delay) => delay,
            CarrierDisposition::Deliver | CarrierDisposition::Duplicate => 0,
            CarrierDisposition::Drop => unreachable!("drop returned above"),
        };
        self.schedule(segment.clone(), delay);
        if disposition == CarrierDisposition::Duplicate {
            self.schedule(segment, delay);
        }
    }

    /// Advances deterministic time and returns all segments whose delay ended.
    pub fn advance(&mut self, ticks: u64) -> Vec<Segment> {
        self.tick = self
            .tick
            .checked_add(ticks)
            .expect("simulator tick exhausted");
        let mut ready = Vec::new();
        let mut waiting = VecDeque::new();
        while let Some(scheduled) = self.scheduled.pop_front() {
            if scheduled.ready_at <= self.tick {
                ready.push(scheduled.segment);
            } else {
                waiting.push_back(scheduled);
            }
        }
        self.scheduled = waiting;
        ready
    }

    pub fn pending(&self) -> usize {
        self.scheduled.len()
    }

    fn schedule(&mut self, segment: Segment, delay: u64) {
        self.scheduled.push_back(ScheduledSegment {
            ready_at: self
                .tick
                .checked_add(delay)
                .expect("simulator tick exhausted"),
            segment,
        });
    }
}

#[cfg(test)]
mod tests {
    use velum_protocol::{Epoch, Sequence};

    use super::*;
    use crate::{Acknowledgement, FlowLimits, ReceiveResult, SessionTracer};

    fn session() -> SessionTracer {
        SessionTracer::new(
            Epoch(0),
            FlowLimits {
                max_flows: 1,
                max_pending_segments: 8,
                max_pending_bytes: 8,
                max_pending_age: 10,
                max_events: 16,
            },
        )
    }

    fn receive_all(
        receiver: &mut SessionTracer,
        segments: Vec<Segment>,
        delivered: &mut Vec<Vec<u8>>,
    ) {
        for segment in segments {
            if let ReceiveResult::Delivered(bytes) = receiver.receive(segment).expect("flow") {
                delivered.push(bytes);
            }
        }
    }

    #[test]
    fn faults_then_ordered_retransmission_deliver_each_segment_once() {
        let mut sender = session();
        let mut receiver = session();
        let sender_flow = sender.open_reliable_flow().expect("open sender flow");
        receiver.open_reliable_flow().expect("open receiver flow");
        let first = sender.send(sender_flow, vec![0]).expect("first");
        let second = sender.send(sender_flow, vec![1]).expect("second");
        let third = sender.send(sender_flow, vec![2]).expect("third");
        let mut carrier = InMemoryCarrier::new();
        let mut delivered = Vec::new();

        carrier.transmit(first, CarrierDisposition::Delay(1));
        carrier.transmit(second, CarrierDisposition::Duplicate);
        carrier.transmit(third, CarrierDisposition::Drop);
        receive_all(&mut receiver, carrier.advance(0), &mut delivered);
        receive_all(&mut receiver, carrier.advance(1), &mut delivered);
        assert_eq!(delivered, vec![vec![0]]);

        sender
            .acknowledge(Acknowledgement {
                flow_id: sender_flow,
                epoch: sender.epoch(),
                through: Sequence(0),
            })
            .expect("first acknowledgement");
        for segment in sender.pending(sender_flow).expect("pending segments") {
            carrier.transmit(segment, CarrierDisposition::Deliver);
        }
        receive_all(&mut receiver, carrier.advance(0), &mut delivered);
        sender
            .acknowledge(Acknowledgement {
                flow_id: sender_flow,
                epoch: sender.epoch(),
                through: Sequence(2),
            })
            .expect("retransmission acknowledgement");

        assert_eq!(delivered, vec![vec![0], vec![1], vec![2]]);
        assert!(
            sender
                .pending(sender_flow)
                .expect("pending segments")
                .is_empty()
        );
    }

    #[test]
    fn black_hole_drops_attempts_until_recovery() {
        let mut carrier = InMemoryCarrier::new();
        let segment = Segment {
            flow_id: velum_protocol::FlowId(0),
            epoch: Epoch(0),
            sequence: Sequence(0),
            bytes: vec![1],
        };

        carrier.set_available(false);
        carrier.transmit(segment.clone(), CarrierDisposition::Deliver);
        assert!(carrier.advance(0).is_empty());

        carrier.set_available(true);
        carrier.transmit(segment.clone(), CarrierDisposition::Deliver);
        assert_eq!(carrier.advance(0), vec![segment]);
    }
}

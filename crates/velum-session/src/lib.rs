//! Deterministic reliable-stream tracer.
//!
//! This model has no networking or frame bytes. It establishes ownership of
//! sequence allocation, cumulative acknowledgement, and receive delivery.

mod simulator;

use std::collections::{BTreeMap, VecDeque};

use velum_protocol::{Epoch, FlowId, Sequence};
use velum_telemetry::SessionEvent;

pub use simulator::{CarrierDisposition, InMemoryCarrier};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Segment {
    pub sequence: Sequence,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FlowLimits {
    pub max_pending_segments: usize,
    pub max_pending_bytes: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TraceError {
    UnknownFlow,
    PendingSegmentLimit,
    PendingByteLimit,
    InvalidAcknowledgement,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReceiveResult {
    Delivered(Vec<u8>),
    Duplicate,
    OutOfOrder,
}

#[derive(Clone, Debug)]
struct ReliableFlow {
    next_send: Sequence,
    next_receive: Sequence,
    pending: VecDeque<Segment>,
    pending_bytes: usize,
}

/// Owns all flow delivery cursors for one logical session.
#[derive(Debug)]
pub struct SessionTracer {
    epoch: Epoch,
    next_flow: u64,
    limits: FlowLimits,
    flows: BTreeMap<FlowId, ReliableFlow>,
    events: Vec<SessionEvent>,
}

impl SessionTracer {
    pub fn new(epoch: Epoch, limits: FlowLimits) -> Self {
        assert!(limits.max_pending_segments > 0);
        assert!(limits.max_pending_bytes > 0);
        Self {
            epoch,
            next_flow: 0,
            limits,
            flows: BTreeMap::new(),
            events: Vec::new(),
        }
    }

    pub fn epoch(&self) -> Epoch {
        self.epoch
    }

    pub fn open_reliable_flow(&mut self) -> FlowId {
        let flow_id = FlowId(self.next_flow);
        self.next_flow = self
            .next_flow
            .checked_add(1)
            .expect("flow identifier exhausted");
        self.flows.insert(
            flow_id,
            ReliableFlow {
                next_send: Sequence(0),
                next_receive: Sequence(0),
                pending: VecDeque::new(),
                pending_bytes: 0,
            },
        );
        self.events.push(SessionEvent::FlowOpened);
        flow_id
    }

    pub fn send(&mut self, flow_id: FlowId, bytes: Vec<u8>) -> Result<Segment, TraceError> {
        let flow = self
            .flows
            .get_mut(&flow_id)
            .ok_or(TraceError::UnknownFlow)?;
        if flow.pending.len() == self.limits.max_pending_segments {
            self.events.push(SessionEvent::PendingLimitReached);
            return Err(TraceError::PendingSegmentLimit);
        }
        if bytes.len()
            > self
                .limits
                .max_pending_bytes
                .saturating_sub(flow.pending_bytes)
        {
            self.events.push(SessionEvent::PendingLimitReached);
            return Err(TraceError::PendingByteLimit);
        }
        let segment = Segment {
            sequence: flow.next_send,
            bytes,
        };
        flow.next_send = Sequence(flow.next_send.0.checked_add(1).expect("sequence exhausted"));
        flow.pending_bytes += segment.bytes.len();
        flow.pending.push_back(segment.clone());
        Ok(segment)
    }

    /// Applies a cumulative acknowledgement for every sequence up to `through`.
    pub fn acknowledge(&mut self, flow_id: FlowId, through: Sequence) -> Result<(), TraceError> {
        let flow = self
            .flows
            .get_mut(&flow_id)
            .ok_or(TraceError::UnknownFlow)?;
        if through.0 >= flow.next_send.0 {
            return Err(TraceError::InvalidAcknowledgement);
        }
        while flow
            .pending
            .front()
            .is_some_and(|segment| segment.sequence <= through)
        {
            let segment = flow.pending.pop_front().expect("front was present");
            flow.pending_bytes -= segment.bytes.len();
        }
        Ok(())
    }

    pub fn receive(
        &mut self,
        flow_id: FlowId,
        segment: Segment,
    ) -> Result<ReceiveResult, TraceError> {
        let flow = self
            .flows
            .get_mut(&flow_id)
            .ok_or(TraceError::UnknownFlow)?;
        if segment.sequence < flow.next_receive {
            self.events.push(SessionEvent::DuplicateIgnored);
            return Ok(ReceiveResult::Duplicate);
        }
        if segment.sequence > flow.next_receive {
            self.events.push(SessionEvent::OutOfOrderRejected);
            return Ok(ReceiveResult::OutOfOrder);
        }
        flow.next_receive = Sequence(
            flow.next_receive
                .0
                .checked_add(1)
                .expect("sequence exhausted"),
        );
        Ok(ReceiveResult::Delivered(segment.bytes))
    }

    pub fn pending(&self, flow_id: FlowId) -> Result<Vec<Segment>, TraceError> {
        self.flows
            .get(&flow_id)
            .map(|flow| flow.pending.iter().cloned().collect())
            .ok_or(TraceError::UnknownFlow)
    }

    pub fn events(&self) -> &[SessionEvent] {
        &self.events
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tracer() -> SessionTracer {
        SessionTracer::new(
            Epoch(0),
            FlowLimits {
                max_pending_segments: 2,
                max_pending_bytes: 3,
            },
        )
    }

    #[test]
    fn reliable_flow_delivers_each_sequence_once() {
        let mut session = tracer();
        let flow = session.open_reliable_flow();
        let first = session.send(flow, vec![1]).expect("send first");
        let second = session.send(flow, vec![2]).expect("send second");
        assert_eq!(
            session.receive(flow, second.clone()),
            Ok(ReceiveResult::OutOfOrder)
        );
        assert_eq!(
            session.receive(flow, first.clone()),
            Ok(ReceiveResult::Delivered(vec![1]))
        );
        assert_eq!(session.receive(flow, first), Ok(ReceiveResult::Duplicate));
        assert_eq!(
            session.receive(flow, second),
            Ok(ReceiveResult::Delivered(vec![2]))
        );
    }

    #[test]
    fn cumulative_acknowledgement_releases_only_confirmed_segments() {
        let mut session = tracer();
        let flow = session.open_reliable_flow();
        session.send(flow, vec![1]).expect("send first");
        session.send(flow, vec![2]).expect("send second");
        session.acknowledge(flow, Sequence(0)).expect("ack first");
        assert_eq!(session.pending(flow).expect("pending").len(), 1);
        assert_eq!(
            session.acknowledge(flow, Sequence(2)),
            Err(TraceError::InvalidAcknowledgement)
        );
    }

    #[test]
    fn pending_memory_is_bounded() {
        let mut session = tracer();
        let flow = session.open_reliable_flow();
        session.send(flow, vec![1, 2, 3]).expect("fill byte budget");
        assert_eq!(
            session.send(flow, vec![4]),
            Err(TraceError::PendingByteLimit)
        );
        session.acknowledge(flow, Sequence(0)).expect("acknowledge");
        session.send(flow, vec![4]).expect("memory released");
    }

    #[test]
    fn exhaustive_short_receive_traces_are_gap_and_duplicate_free() {
        const INPUTS: [u64; 3] = [0, 1, 2];

        fn explore(prefix: &mut Vec<u64>, depth: usize) {
            if depth == 0 {
                let mut session = tracer();
                let flow = session.open_reliable_flow();
                let mut delivered = Vec::new();
                for sequence in prefix.iter().copied() {
                    if let ReceiveResult::Delivered(bytes) = session
                        .receive(
                            flow,
                            Segment {
                                sequence: Sequence(sequence),
                                bytes: vec![sequence as u8],
                            },
                        )
                        .expect("known flow")
                    {
                        delivered.push(bytes[0]);
                    }
                }
                assert_eq!(delivered, (0..delivered.len() as u8).collect::<Vec<_>>());
                return;
            }
            for input in INPUTS {
                prefix.push(input);
                explore(prefix, depth - 1);
                prefix.pop();
            }
        }

        for length in 0..=4 {
            explore(&mut Vec::new(), length);
        }
    }
}

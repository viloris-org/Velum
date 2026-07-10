//! Deterministic reliable-stream tracer.
//!
//! This model has no networking or frame bytes. It establishes ownership of
//! sequence allocation, cumulative acknowledgement, and receive delivery.

mod simulator;
mod transition;

#[cfg(test)]
mod campaign;

use std::collections::{BTreeMap, VecDeque};

use transition::{EpochValidity, TransitionState};
use velum_protocol::{Epoch, FlowId, Sequence};
use velum_telemetry::SessionEvent;

pub use simulator::{CarrierDisposition, InMemoryCarrier};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Segment {
    pub flow_id: FlowId,
    pub epoch: Epoch,
    pub sequence: Sequence,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Acknowledgement {
    pub flow_id: FlowId,
    pub epoch: Epoch,
    pub through: Sequence,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FlowLimits {
    pub max_pending_segments: usize,
    pub max_pending_bytes: usize,
    pub max_pending_age: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TraceError {
    UnknownFlow,
    PendingSegmentLimit,
    PendingByteLimit,
    FlowTimedOut,
    InvalidAcknowledgement,
    StaleEpoch,
    FutureEpoch,
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
    pending: VecDeque<PendingSegment>,
    pending_bytes: usize,
    timed_out: bool,
}

#[derive(Clone, Debug)]
struct PendingSegment {
    segment: Segment,
    expires_at: u64,
}

/// Owns all flow delivery cursors for one logical session.
#[derive(Debug)]
pub struct SessionTracer {
    transition: TransitionState,
    tick: u64,
    next_flow: u64,
    limits: FlowLimits,
    flows: BTreeMap<FlowId, ReliableFlow>,
    events: Vec<SessionEvent>,
}

impl SessionTracer {
    pub fn new(epoch: Epoch, limits: FlowLimits) -> Self {
        assert!(limits.max_pending_segments > 0);
        assert!(limits.max_pending_bytes > 0);
        assert!(limits.max_pending_age > 0);
        Self {
            transition: TransitionState::new(epoch),
            tick: 0,
            next_flow: 0,
            limits,
            flows: BTreeMap::new(),
            events: Vec::new(),
        }
    }

    pub fn epoch(&self) -> Epoch {
        self.transition.current()
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
                timed_out: false,
            },
        );
        self.events.push(SessionEvent::FlowOpened);
        flow_id
    }

    pub fn send(&mut self, flow_id: FlowId, bytes: Vec<u8>) -> Result<Segment, TraceError> {
        let epoch = self.epoch();
        let flow = self
            .flows
            .get_mut(&flow_id)
            .ok_or(TraceError::UnknownFlow)?;
        if flow.timed_out {
            return Err(TraceError::FlowTimedOut);
        }
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
            flow_id,
            epoch,
            sequence: flow.next_send,
            bytes,
        };
        flow.next_send = Sequence(flow.next_send.0.checked_add(1).expect("sequence exhausted"));
        flow.pending_bytes += segment.bytes.len();
        flow.pending.push_back(PendingSegment {
            segment: segment.clone(),
            expires_at: self
                .tick
                .checked_add(self.limits.max_pending_age)
                .expect("pending timeout exhausted"),
        });
        Ok(segment)
    }

    /// Applies a cumulative acknowledgement for every sequence up to `through`.
    pub fn acknowledge(&mut self, acknowledgement: Acknowledgement) -> Result<(), TraceError> {
        self.require_valid_epoch(acknowledgement.epoch)?;
        let flow = self
            .flows
            .get_mut(&acknowledgement.flow_id)
            .ok_or(TraceError::UnknownFlow)?;
        if flow.timed_out {
            return Err(TraceError::FlowTimedOut);
        }
        if acknowledgement.through.0 >= flow.next_send.0 {
            return Err(TraceError::InvalidAcknowledgement);
        }
        while flow
            .pending
            .front()
            .is_some_and(|pending| pending.segment.sequence <= acknowledgement.through)
        {
            let pending = flow.pending.pop_front().expect("front was present");
            flow.pending_bytes -= pending.segment.bytes.len();
        }
        Ok(())
    }

    pub fn receive(&mut self, segment: Segment) -> Result<ReceiveResult, TraceError> {
        self.require_valid_epoch(segment.epoch)?;
        let flow = self
            .flows
            .get_mut(&segment.flow_id)
            .ok_or(TraceError::UnknownFlow)?;
        if flow.timed_out {
            return Err(TraceError::FlowTimedOut);
        }
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
            .map(|flow| {
                flow.pending
                    .iter()
                    .map(|pending| pending.segment.clone())
                    .collect()
            })
            .ok_or(TraceError::UnknownFlow)
    }

    /// Advances deterministic time and terminates flows whose oldest
    /// unacknowledged segment exceeded its configured lifetime.
    pub fn advance_time(&mut self, ticks: u64) -> Vec<FlowId> {
        self.tick = self
            .tick
            .checked_add(ticks)
            .expect("session tick exhausted");
        let mut timed_out = Vec::new();
        for (flow_id, flow) in &mut self.flows {
            if !flow.timed_out
                && flow
                    .pending
                    .front()
                    .is_some_and(|pending| pending.expires_at <= self.tick)
            {
                flow.pending.clear();
                flow.pending_bytes = 0;
                flow.timed_out = true;
                timed_out.push(*flow_id);
                self.events.push(SessionEvent::FlowTimedOut);
            }
        }
        timed_out
    }

    pub fn events(&self) -> &[SessionEvent] {
        &self.events
    }

    /// Opens a one-epoch receive window for segments and acknowledgements
    /// still in flight on the retiring carrier.
    pub fn begin_transition(&mut self) -> Epoch {
        self.transition.advance()
    }

    /// Closes the retiring epoch's replay window after its carrier detaches.
    pub fn complete_transition(&mut self) {
        self.transition.retire_previous();
    }

    fn require_valid_epoch(&self, epoch: Epoch) -> Result<(), TraceError> {
        match self.transition.validate(epoch) {
            EpochValidity::Current | EpochValidity::Retiring => Ok(()),
            EpochValidity::Stale => Err(TraceError::StaleEpoch),
            EpochValidity::Future => Err(TraceError::FutureEpoch),
        }
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
                max_pending_age: 2,
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
            session.receive(second.clone()),
            Ok(ReceiveResult::OutOfOrder)
        );
        assert_eq!(
            session.receive(first.clone()),
            Ok(ReceiveResult::Delivered(vec![1]))
        );
        assert_eq!(session.receive(first), Ok(ReceiveResult::Duplicate));
        assert_eq!(
            session.receive(second),
            Ok(ReceiveResult::Delivered(vec![2]))
        );
    }

    #[test]
    fn cumulative_acknowledgement_releases_only_confirmed_segments() {
        let mut session = tracer();
        let flow = session.open_reliable_flow();
        session.send(flow, vec![1]).expect("send first");
        session.send(flow, vec![2]).expect("send second");
        session
            .acknowledge(Acknowledgement {
                flow_id: flow,
                epoch: session.epoch(),
                through: Sequence(0),
            })
            .expect("ack first");
        assert_eq!(session.pending(flow).expect("pending").len(), 1);
        assert_eq!(
            session.acknowledge(Acknowledgement {
                flow_id: flow,
                epoch: session.epoch(),
                through: Sequence(2),
            }),
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
        session
            .acknowledge(Acknowledgement {
                flow_id: flow,
                epoch: session.epoch(),
                through: Sequence(0),
            })
            .expect("acknowledge");
        session.send(flow, vec![4]).expect("memory released");
    }

    #[test]
    fn pending_timeout_releases_memory_by_terminating_the_flow() {
        let mut session = tracer();
        let flow = session.open_reliable_flow();
        session.send(flow, vec![1, 2, 3]).expect("send");
        assert!(session.advance_time(1).is_empty());
        assert_eq!(session.advance_time(1), vec![flow]);
        assert!(session.pending(flow).expect("pending").is_empty());
        assert_eq!(session.send(flow, vec![4]), Err(TraceError::FlowTimedOut));
        assert_eq!(
            session.events(),
            &[SessionEvent::FlowOpened, SessionEvent::FlowTimedOut]
        );
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
                        .receive(Segment {
                            flow_id: flow,
                            epoch: session.epoch(),
                            sequence: Sequence(sequence),
                            bytes: vec![sequence as u8],
                        })
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

    #[test]
    fn flow_identity_and_epoch_bound_delivery_during_transition() {
        let mut session = tracer();
        let flow = session.open_reliable_flow();
        let first = session.send(flow, vec![1]).expect("first");
        assert_eq!(first.flow_id, flow);
        assert_eq!(first.epoch, Epoch(0));

        assert_eq!(session.begin_transition(), Epoch(1));
        assert_eq!(
            session.receive(first.clone()),
            Ok(ReceiveResult::Delivered(vec![1]))
        );
        session.complete_transition();
        assert_eq!(session.receive(first), Err(TraceError::StaleEpoch));
    }

    #[test]
    fn acknowledgements_use_the_same_bounded_epoch_window() {
        let mut session = tracer();
        let flow = session.open_reliable_flow();
        session.send(flow, vec![1]).expect("first");
        session.begin_transition();
        session
            .acknowledge(Acknowledgement {
                flow_id: flow,
                epoch: Epoch(0),
                through: Sequence(0),
            })
            .expect("retiring epoch acknowledgement");
        session.complete_transition();
        assert_eq!(
            session.acknowledge(Acknowledgement {
                flow_id: flow,
                epoch: Epoch(0),
                through: Sequence(0),
            }),
            Err(TraceError::StaleEpoch)
        );
    }
}

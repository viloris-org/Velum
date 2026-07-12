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
use velum_carrier_api::{CarrierId, CarrierKind};
use velum_crypto::{
    AttachmentAuthenticationError, AttachmentAuthenticator, AttachmentProof, ReplayWindow,
};
use velum_protocol::{Epoch, FlowId, Sequence, SessionId, VersionRange};
use velum_telemetry::{CarrierClass, SessionEvent, TransitionReason, TransitionRejection};

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
    pub max_flows: usize,
    pub max_pending_segments: usize,
    pub max_pending_bytes: usize,
    pub max_pending_age: u64,
    pub max_events: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TraceError {
    UnknownFlow,
    FlowLimit,
    PendingSegmentLimit,
    PendingByteLimit,
    FlowHasPendingSegments,
    InvalidAcknowledgement,
    TransitionInProgress,
    StaleEpoch,
    FutureEpoch,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CarrierAttachment {
    pub carrier: CarrierId,
    pub epoch: Epoch,
    pub proof: AttachmentProof,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AttachmentError {
    Authentication(AttachmentAuthenticationError),
    StaleEpoch,
    FutureEpoch,
    Replay,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TransitionCompatibilityError {
    NoSharedVersion,
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
    attachment_replay: ReplayWindow,
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
        assert!(limits.max_flows > 0);
        assert!(limits.max_events > 0);
        Self {
            transition: TransitionState::new(epoch),
            attachment_replay: ReplayWindow::default(),
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

    pub fn open_reliable_flow(&mut self) -> Result<FlowId, TraceError> {
        if self.flows.len() >= self.limits.max_flows {
            return Err(TraceError::FlowLimit);
        }
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
        self.record_event(SessionEvent::FlowOpened);
        Ok(flow_id)
    }

    /// Releases a completed or timed-out flow slot. Callers must retain any
    /// late-packet handling they require before closing the flow.
    pub fn close_reliable_flow(&mut self, flow_id: FlowId) -> Result<(), TraceError> {
        let flow = self.flows.get(&flow_id).ok_or(TraceError::UnknownFlow)?;
        if !flow.pending.is_empty() {
            return Err(TraceError::FlowHasPendingSegments);
        }
        self.flows.remove(&flow_id);
        Ok(())
    }

    pub fn send(&mut self, flow_id: FlowId, bytes: Vec<u8>) -> Result<Segment, TraceError> {
        let epoch = self.epoch();
        let flow = self
            .flows
            .get_mut(&flow_id)
            .ok_or(TraceError::UnknownFlow)?;
        if flow.pending.len() == self.limits.max_pending_segments {
            self.record_event(SessionEvent::PendingLimitReached);
            return Err(TraceError::PendingSegmentLimit);
        }
        if bytes.len()
            > self
                .limits
                .max_pending_bytes
                .saturating_sub(flow.pending_bytes)
        {
            self.record_event(SessionEvent::PendingLimitReached);
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
        if acknowledgement.through.0 >= flow.next_send.0 {
            return Err(TraceError::InvalidAcknowledgement);
        }
        if flow.pending.iter().any(|pending| {
            pending.segment.sequence <= acknowledgement.through
                && pending.segment.epoch > acknowledgement.epoch
        }) {
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
        if segment.sequence < flow.next_receive {
            self.record_event(SessionEvent::DuplicateIgnored);
            return Ok(ReceiveResult::Duplicate);
        }
        if segment.sequence > flow.next_receive {
            self.record_event(SessionEvent::OutOfOrderRejected);
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

    /// Reissues every unacknowledged segment on the current epoch after a
    /// carrier attachment succeeds. Sequence numbers remain session-owned;
    /// receivers therefore classify already delivered bytes as duplicates.
    pub fn resume_unacknowledged(&self, flow_id: FlowId) -> Result<Vec<Segment>, TraceError> {
        self.flows
            .get(&flow_id)
            .map(|flow| {
                flow.pending
                    .iter()
                    .map(|pending| Segment {
                        epoch: self.epoch(),
                        ..pending.segment.clone()
                    })
                    .collect()
            })
            .ok_or(TraceError::UnknownFlow)
    }

    /// Advances deterministic time and removes flows whose oldest
    /// unacknowledged segment exceeded its configured lifetime.
    pub fn advance_time(&mut self, ticks: u64) -> Vec<FlowId> {
        self.tick = self
            .tick
            .checked_add(ticks)
            .expect("session tick exhausted");
        let mut timed_out = Vec::new();
        for (flow_id, flow) in &self.flows {
            if flow
                .pending
                .front()
                .is_some_and(|pending| pending.expires_at <= self.tick)
            {
                timed_out.push(*flow_id);
            }
        }
        for flow_id in &timed_out {
            self.flows.remove(flow_id);
        }
        for _ in &timed_out {
            self.record_event(SessionEvent::FlowTimedOut);
        }
        timed_out
    }

    pub fn events(&self) -> &[SessionEvent] {
        &self.events
    }

    /// Opens a one-epoch receive window for segments and acknowledgements
    /// still in flight on the retiring carrier.
    pub fn begin_transition(&mut self) -> Result<Epoch, TraceError> {
        self.transition
            .advance()
            .ok_or(TraceError::TransitionInProgress)
    }

    /// Records an explainable carrier change without exporting transport
    /// addresses, error text, credentials, or payload-derived data.
    pub fn begin_explained_transition(
        &mut self,
        from: CarrierKind,
        to: CarrierKind,
        reason: TransitionReason,
    ) -> Result<Epoch, TraceError> {
        let epoch = self.begin_transition()?;
        self.record_event(SessionEvent::TransitionStarted {
            from: carrier_class(from),
            to: carrier_class(to),
            reason,
        });
        Ok(epoch)
    }

    pub fn negotiate_transition_version(
        &mut self,
        local: VersionRange,
        peer: VersionRange,
    ) -> Result<u16, TransitionCompatibilityError> {
        local.negotiate(peer).ok_or_else(|| {
            self.record_event(SessionEvent::TransitionRejected {
                reason: TransitionRejection::IncompatibleVersion,
            });
            TransitionCompatibilityError::NoSharedVersion
        })
    }

    /// Accepts a new carrier only when its proof binds it to this session's
    /// current epoch and that epoch has not already been attached.
    pub fn authenticate_carrier_attachment(
        &mut self,
        authenticator: &AttachmentAuthenticator,
        session: SessionId,
        attachment: CarrierAttachment,
    ) -> Result<(), AttachmentError> {
        match self.transition.validate(attachment.epoch) {
            EpochValidity::Current => {}
            EpochValidity::Retiring | EpochValidity::Stale => {
                return self.reject_attachment(AttachmentError::StaleEpoch);
            }
            EpochValidity::Future => return self.reject_attachment(AttachmentError::FutureEpoch),
        }
        authenticator
            .verify(
                session,
                attachment.carrier.0,
                attachment.epoch,
                attachment.proof,
            )
            .map_err(AttachmentError::Authentication)
            .or_else(|error| self.reject_attachment(error))?;
        if !self.attachment_replay.accept(attachment.epoch) {
            return self.reject_attachment(AttachmentError::Replay);
        }
        Ok(())
    }

    /// Closes the retiring epoch's replay window after its carrier detaches.
    pub fn complete_transition(&mut self) {
        self.transition.retire_previous();
        self.record_event(SessionEvent::TransitionCompleted);
    }

    fn require_valid_epoch(&self, epoch: Epoch) -> Result<(), TraceError> {
        match self.transition.validate(epoch) {
            EpochValidity::Current | EpochValidity::Retiring => Ok(()),
            EpochValidity::Stale => Err(TraceError::StaleEpoch),
            EpochValidity::Future => Err(TraceError::FutureEpoch),
        }
    }

    fn reject_attachment<T>(&mut self, error: AttachmentError) -> Result<T, AttachmentError> {
        self.record_event(SessionEvent::TransitionRejected {
            reason: TransitionRejection::AttachmentRejected,
        });
        Err(error)
    }

    fn record_event(&mut self, event: SessionEvent) {
        if self.events.len() >= self.limits.max_events {
            self.events.remove(0);
        }
        self.events.push(event);
    }
}

fn carrier_class(kind: CarrierKind) -> CarrierClass {
    match kind {
        CarrierKind::Quic => CarrierClass::Quic,
        CarrierKind::Tls => CarrierClass::Tls,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tracer() -> SessionTracer {
        SessionTracer::new(
            Epoch(0),
            FlowLimits {
                max_flows: 2,
                max_pending_segments: 2,
                max_pending_bytes: 3,
                max_pending_age: 2,
                max_events: 2,
            },
        )
    }

    fn attachment(
        epoch: Epoch,
        carrier: CarrierId,
    ) -> (AttachmentAuthenticator, SessionId, CarrierAttachment) {
        let authenticator = AttachmentAuthenticator::new(b"attachment secret").expect("secret");
        let session = SessionId([5; 16]);
        let attachment = CarrierAttachment {
            carrier,
            epoch,
            proof: authenticator.prove(session, carrier.0, epoch),
        };
        (authenticator, session, attachment)
    }

    #[test]
    fn carrier_attachments_require_a_fresh_authenticated_current_epoch() {
        let mut session = tracer();
        let (authenticator, session_id, first) = attachment(Epoch(0), CarrierId(1));
        assert_eq!(
            session.authenticate_carrier_attachment(&authenticator, session_id, first),
            Ok(())
        );
        assert_eq!(
            session.authenticate_carrier_attachment(&authenticator, session_id, first),
            Err(AttachmentError::Replay)
        );
        assert_eq!(
            session.events(),
            &[SessionEvent::TransitionRejected {
                reason: TransitionRejection::AttachmentRejected,
            }]
        );

        assert_eq!(session.begin_transition(), Ok(Epoch(1)));
        let (_, _, second) = attachment(Epoch(1), CarrierId(2));
        assert_eq!(
            session.authenticate_carrier_attachment(&authenticator, session_id, second),
            Ok(())
        );
    }

    #[test]
    fn carrier_attachments_reject_stale_future_and_forged_proofs() {
        let mut session = tracer();
        let (authenticator, session_id, first) = attachment(Epoch(0), CarrierId(1));
        let forged = CarrierAttachment {
            carrier: CarrierId(2),
            ..first
        };
        assert!(matches!(
            session.authenticate_carrier_attachment(&authenticator, session_id, forged),
            Err(AttachmentError::Authentication(
                AttachmentAuthenticationError::InvalidProof
            ))
        ));
        assert_eq!(session.begin_transition(), Ok(Epoch(1)));
        assert_eq!(
            session.authenticate_carrier_attachment(&authenticator, session_id, first),
            Err(AttachmentError::StaleEpoch)
        );
        let (_, _, future) = attachment(Epoch(2), CarrierId(3));
        assert_eq!(
            session.authenticate_carrier_attachment(&authenticator, session_id, future),
            Err(AttachmentError::FutureEpoch)
        );
    }

    #[test]
    fn transitions_cannot_overlap_the_retiring_epoch_window() {
        let mut session = tracer();
        assert_eq!(session.begin_transition(), Ok(Epoch(1)));
        assert_eq!(
            session.begin_transition(),
            Err(TraceError::TransitionInProgress)
        );
    }

    #[test]
    fn reliable_flow_delivers_each_sequence_once() {
        let mut session = tracer();
        let flow = session.open_reliable_flow().expect("open flow");
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
        let flow = session.open_reliable_flow().expect("open flow");
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
        let flow = session.open_reliable_flow().expect("open flow");
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
    fn pending_timeout_removes_the_flow_and_releases_its_session_slot() {
        let mut session = SessionTracer::new(
            Epoch(0),
            FlowLimits {
                max_flows: 1,
                max_pending_segments: 2,
                max_pending_bytes: 3,
                max_pending_age: 2,
                max_events: 3,
            },
        );
        let flow = session.open_reliable_flow().expect("open flow");
        session.send(flow, vec![1, 2, 3]).expect("send");
        assert!(session.advance_time(1).is_empty());
        assert_eq!(session.advance_time(1), vec![flow]);
        assert_eq!(session.pending(flow), Err(TraceError::UnknownFlow));
        assert_eq!(session.open_reliable_flow(), Ok(FlowId(1)));
        assert_eq!(
            session.events(),
            &[
                SessionEvent::FlowOpened,
                SessionEvent::FlowTimedOut,
                SessionEvent::FlowOpened,
            ]
        );
    }

    #[test]
    fn closed_flows_release_their_session_slot() {
        let mut session = SessionTracer::new(
            Epoch(0),
            FlowLimits {
                max_flows: 1,
                max_pending_segments: 1,
                max_pending_bytes: 1,
                max_pending_age: 1,
                max_events: 1,
            },
        );
        let first = session.open_reliable_flow().expect("first flow");
        session.close_reliable_flow(first).expect("close flow");
        assert_eq!(session.open_reliable_flow(), Ok(FlowId(1)));
    }

    #[test]
    fn exhaustive_short_receive_traces_are_gap_and_duplicate_free() {
        const INPUTS: [u64; 3] = [0, 1, 2];

        fn explore(prefix: &mut Vec<u64>, depth: usize) {
            if depth == 0 {
                let mut session = tracer();
                let flow = session.open_reliable_flow().expect("open flow");
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
        let flow = session.open_reliable_flow().expect("open flow");
        let first = session.send(flow, vec![1]).expect("first");
        assert_eq!(first.flow_id, flow);
        assert_eq!(first.epoch, Epoch(0));

        assert_eq!(session.begin_transition(), Ok(Epoch(1)));
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
        let flow = session.open_reliable_flow().expect("open flow");
        session.send(flow, vec![1]).expect("first");
        session.begin_transition().expect("begin transition");
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

    #[test]
    fn resume_reissues_only_unacknowledged_segments_on_current_epoch() {
        let mut session = tracer();
        let flow = session.open_reliable_flow().expect("open flow");
        session.send(flow, vec![1]).expect("first");
        session.send(flow, vec![2]).expect("second");
        session
            .acknowledge(Acknowledgement {
                flow_id: flow,
                epoch: Epoch(0),
                through: Sequence(0),
            })
            .expect("acknowledge first");
        assert_eq!(session.begin_transition(), Ok(Epoch(1)));
        assert_eq!(
            session.resume_unacknowledged(flow).expect("resume"),
            vec![Segment {
                flow_id: flow,
                epoch: Epoch(1),
                sequence: Sequence(1),
                bytes: vec![2],
            }]
        );
    }

    #[test]
    fn incompatible_transition_versions_are_rejected_and_explained() {
        let mut session = tracer();
        assert_eq!(
            session.negotiate_transition_version(
                VersionRange {
                    minimum: 0,
                    maximum: 0
                },
                VersionRange {
                    minimum: 1,
                    maximum: 1
                },
            ),
            Err(TransitionCompatibilityError::NoSharedVersion)
        );
        assert_eq!(
            session.events(),
            &[SessionEvent::TransitionRejected {
                reason: TransitionRejection::IncompatibleVersion,
            }]
        );
    }

    #[test]
    fn transition_telemetry_has_only_structured_classes() {
        let mut session = tracer();
        assert_eq!(
            session.begin_explained_transition(
                CarrierKind::Quic,
                CarrierKind::Tls,
                TransitionReason::PrimaryUnhealthy,
            ),
            Ok(Epoch(1))
        );
        session.complete_transition();
        assert_eq!(
            session.events(),
            &[
                SessionEvent::TransitionStarted {
                    from: CarrierClass::Quic,
                    to: CarrierClass::Tls,
                    reason: TransitionReason::PrimaryUnhealthy,
                },
                SessionEvent::TransitionCompleted,
            ]
        );
    }

    #[test]
    fn retiring_epoch_acknowledgement_cannot_acknowledge_current_epoch_segments() {
        let mut session = tracer();
        let flow = session.open_reliable_flow().expect("open flow");
        session.send(flow, vec![0]).expect("old epoch segment");
        assert_eq!(session.begin_transition(), Ok(Epoch(1)));
        session.send(flow, vec![1]).expect("current epoch segment");

        assert_eq!(
            session.acknowledge(Acknowledgement {
                flow_id: flow,
                epoch: Epoch(0),
                through: Sequence(1),
            }),
            Err(TraceError::InvalidAcknowledgement)
        );
        assert_eq!(session.pending(flow).expect("pending").len(), 2);
    }

    #[test]
    fn session_metadata_is_bounded() {
        let mut session = SessionTracer::new(
            Epoch(0),
            FlowLimits {
                max_flows: 1,
                max_pending_segments: 2,
                max_pending_bytes: 2,
                max_pending_age: 2,
                max_events: 2,
            },
        );
        let flow = session.open_reliable_flow().expect("first flow");
        assert_eq!(session.open_reliable_flow(), Err(TraceError::FlowLimit));

        let segment = session.send(flow, vec![0]).expect("segment");
        assert_eq!(
            session.receive(segment.clone()),
            Ok(ReceiveResult::Delivered(vec![0]))
        );
        assert_eq!(
            session.receive(segment.clone()),
            Ok(ReceiveResult::Duplicate)
        );
        assert_eq!(session.receive(segment), Ok(ReceiveResult::Duplicate));
        assert_eq!(session.events().len(), 2);
    }
}

//! Maps authenticated v0 frames into one in-memory session tracer.

use velum_carrier_api::CarrierId;
use velum_crypto::{AttachmentAuthenticator, AttachmentProof};
use velum_protocol::{Frame, ProtocolErrorCode, SessionId};

use crate::{
    Acknowledgement, AttachmentAcceptance, AttachmentError, CarrierAttachment, ReceiveResult,
    Segment, SessionTracer, TraceError,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DispatchOutcome {
    AttachmentAccepted { response: Frame },
    AttachmentAlreadyAccepted { response: Frame },
    FlowOpened,
    Acknowledged,
    Delivered(Vec<u8>),
    Duplicate,
    OutOfOrder,
    FlowFinished,
    FlowReset { error: ProtocolErrorCode },
    ConnectionClosed { error: ProtocolErrorCode },
    OptionalFrameIgnored,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DispatchError {
    Closed,
    CarrierNotAttached,
    SessionMismatch,
    UnexpectedFrame,
    Attachment(AttachmentError),
    Trace(TraceError),
}

impl DispatchError {
    pub const fn error_code(self) -> ProtocolErrorCode {
        match self {
            Self::Closed
            | Self::CarrierNotAttached
            | Self::SessionMismatch
            | Self::UnexpectedFrame => ProtocolErrorCode::ProtocolViolation,
            Self::Attachment(AttachmentError::Authentication(_))
            | Self::Attachment(AttachmentError::NegotiationMismatch)
            | Self::Attachment(AttachmentError::MissingNegotiation) => {
                ProtocolErrorCode::AuthenticationFailed
            }
            Self::Attachment(AttachmentError::Replay) => ProtocolErrorCode::ReplayDetected,
            Self::Attachment(AttachmentError::StaleEpoch)
            | Self::Attachment(AttachmentError::FutureEpoch) => {
                ProtocolErrorCode::ProtocolViolation
            }
            Self::Trace(TraceError::UnknownFlow) => ProtocolErrorCode::FlowNotFound,
            Self::Trace(TraceError::FlowLimit)
            | Self::Trace(TraceError::PendingSegmentLimit)
            | Self::Trace(TraceError::PendingByteLimit)
            | Self::Trace(TraceError::SessionPendingSegmentLimit)
            | Self::Trace(TraceError::SessionPendingByteLimit) => ProtocolErrorCode::ResourceLimit,
            Self::Trace(_) => ProtocolErrorCode::FlowState,
        }
    }
}

/// Per-carrier dispatch state. The caller supplies the session explicitly so
/// session ownership stays with its lifecycle manager rather than a socket.
pub struct SessionFrameDispatcher<'a> {
    session_id: SessionId,
    carrier: CarrierId,
    authenticator: &'a AttachmentAuthenticator,
    closed: bool,
}

impl<'a> SessionFrameDispatcher<'a> {
    pub const fn new(
        session_id: SessionId,
        carrier: CarrierId,
        authenticator: &'a AttachmentAuthenticator,
    ) -> Self {
        Self {
            session_id,
            carrier,
            authenticator,
            closed: false,
        }
    }

    pub const fn is_closed(&self) -> bool {
        self.closed
    }

    pub fn dispatch(
        &mut self,
        session: &mut SessionTracer,
        frame: Frame,
    ) -> Result<DispatchOutcome, DispatchError> {
        if self.closed {
            return Err(DispatchError::Closed);
        }
        match frame {
            Frame::Attach {
                session_id,
                epoch,
                attachment_id,
                parameters,
                proof,
            } => self.attach(session, session_id, epoch, attachment_id, parameters, proof),
            Frame::StreamOpen { flow_id } => {
                self.require_active_attachment(session)?;
                session
                    .open_reliable_flow_with_id(flow_id)
                    .map_err(DispatchError::Trace)?;
                Ok(DispatchOutcome::FlowOpened)
            }
            Frame::StreamData {
                flow_id,
                epoch,
                sequence,
                bytes,
            } => {
                self.require_dispatchable_attachment(session)?;
                match session
                    .receive(Segment {
                        flow_id,
                        epoch,
                        sequence,
                        bytes,
                    })
                    .map_err(DispatchError::Trace)?
                {
                    ReceiveResult::Delivered(bytes) => Ok(DispatchOutcome::Delivered(bytes)),
                    ReceiveResult::Duplicate => Ok(DispatchOutcome::Duplicate),
                    ReceiveResult::OutOfOrder => Ok(DispatchOutcome::OutOfOrder),
                }
            }
            Frame::StreamAcknowledgement {
                flow_id,
                epoch,
                through,
            } => {
                self.require_dispatchable_attachment(session)?;
                session
                    .acknowledge(Acknowledgement {
                        flow_id,
                        epoch,
                        through,
                    })
                    .map_err(DispatchError::Trace)?;
                Ok(DispatchOutcome::Acknowledged)
            }
            Frame::StreamFinish {
                flow_id,
                epoch,
                final_next_sequence,
            } => {
                self.require_dispatchable_attachment(session)?;
                session
                    .finish_receiving(flow_id, epoch, final_next_sequence)
                    .map_err(DispatchError::Trace)?;
                Ok(DispatchOutcome::FlowFinished)
            }
            Frame::StreamReset { flow_id, error } => {
                self.require_dispatchable_attachment(session)?;
                session
                    .reset_reliable_flow(flow_id)
                    .map_err(DispatchError::Trace)?;
                Ok(DispatchOutcome::FlowReset { error })
            }
            Frame::ConnectionClose { error } => {
                self.require_dispatchable_attachment(session)?;
                self.closed = true;
                Ok(DispatchOutcome::ConnectionClosed { error })
            }
            Frame::UnknownOptional { .. } => {
                self.require_dispatchable_attachment(session)?;
                Ok(DispatchOutcome::OptionalFrameIgnored)
            }
            Frame::NegotiationOffer(_)
            | Frame::NegotiationAccept(_)
            | Frame::AttachAccepted { .. } => Err(DispatchError::UnexpectedFrame),
        }
    }

    fn attach(
        &self,
        session: &mut SessionTracer,
        session_id: SessionId,
        epoch: velum_protocol::Epoch,
        attachment_id: velum_protocol::AttachmentId,
        parameters: velum_protocol::NegotiatedParameters,
        proof: [u8; velum_crypto::ATTACHMENT_PROOF_BYTES],
    ) -> Result<DispatchOutcome, DispatchError> {
        if session_id != self.session_id {
            return Err(DispatchError::SessionMismatch);
        }
        let acceptance = session
            .authenticate_carrier_attachment(
                self.authenticator,
                self.session_id,
                CarrierAttachment {
                    carrier: self.carrier,
                    epoch,
                    attachment_id,
                    parameters,
                    proof: AttachmentProof::from_bytes(proof),
                },
            )
            .map_err(DispatchError::Attachment)?;
        let response = Frame::AttachAccepted {
            session_id: self.session_id,
            epoch,
            attachment_id,
        };
        match acceptance {
            AttachmentAcceptance::Accepted => Ok(DispatchOutcome::AttachmentAccepted { response }),
            AttachmentAcceptance::AlreadyAccepted => {
                Ok(DispatchOutcome::AttachmentAlreadyAccepted { response })
            }
        }
    }

    fn require_active_attachment(&self, session: &SessionTracer) -> Result<(), DispatchError> {
        session
            .carrier_attachment_is_active(self.carrier)
            .then_some(())
            .ok_or(DispatchError::CarrierNotAttached)
    }

    fn require_dispatchable_attachment(
        &self,
        session: &SessionTracer,
    ) -> Result<(), DispatchError> {
        session
            .carrier_attachment_is_dispatchable(self.carrier)
            .then_some(())
            .ok_or(DispatchError::CarrierNotAttached)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use velum_protocol::{
        AttachmentId, Capabilities, CapabilityAdvertisement, Epoch, FlowId, HandshakeNonce,
        NegotiationOffer, Sequence, VersionRange,
    };

    fn parameters() -> velum_protocol::NegotiatedParameters {
        NegotiationOffer::new(
            VersionRange::new(0, 0).expect("range"),
            CapabilityAdvertisement::new(Capabilities::RELIABLE_STREAM, Capabilities(0))
                .expect("capabilities"),
            HandshakeNonce::new([1; 16]).expect("nonce"),
        )
        .expect("offer")
        .negotiate(
            VersionRange::new(0, 0).expect("range"),
            Capabilities::RELIABLE_STREAM,
            HandshakeNonce::new([2; 16]).expect("nonce"),
        )
        .expect("parameters")
    }

    fn tracer() -> SessionTracer {
        SessionTracer::new(
            Epoch(0),
            crate::FlowLimits {
                max_flows: 4,
                max_pending_segments: 4,
                max_pending_bytes: 64,
                max_session_pending_segments: 8,
                max_session_pending_bytes: 128,
                max_pending_age: 4,
                max_events: 8,
            },
        )
    }

    fn attach_frame(authenticator: &AttachmentAuthenticator, session_id: SessionId) -> Frame {
        attach_frame_at(authenticator, session_id, Epoch(0), [3; 16])
    }

    fn attach_frame_at(
        authenticator: &AttachmentAuthenticator,
        session_id: SessionId,
        epoch: Epoch,
        attachment_bytes: [u8; 16],
    ) -> Frame {
        let parameters = parameters();
        let attachment_id = AttachmentId::new(attachment_bytes).expect("attachment id");
        Frame::Attach {
            session_id,
            epoch,
            attachment_id,
            parameters,
            proof: authenticator
                .prove(session_id, epoch, attachment_id, parameters)
                .bytes(),
        }
    }

    #[test]
    fn attachment_gates_stream_dispatch_and_is_idempotent() {
        let authenticator = AttachmentAuthenticator::new(b"secret").expect("secret");
        let session_id = SessionId([9; 16]);
        let mut session = tracer();
        session
            .bind_negotiated_parameters(parameters())
            .expect("bind parameters");
        let mut dispatcher = SessionFrameDispatcher::new(session_id, CarrierId(4), &authenticator);

        assert_eq!(
            dispatcher.dispatch(&mut session, Frame::StreamOpen { flow_id: FlowId(5) }),
            Err(DispatchError::CarrierNotAttached)
        );
        let attach = attach_frame(&authenticator, session_id);
        assert!(matches!(
            dispatcher.dispatch(&mut session, attach.clone()),
            Ok(DispatchOutcome::AttachmentAccepted { .. })
        ));
        assert!(matches!(
            dispatcher.dispatch(&mut session, attach),
            Ok(DispatchOutcome::AttachmentAlreadyAccepted { .. })
        ));
        assert_eq!(
            dispatcher.dispatch(&mut session, Frame::StreamOpen { flow_id: FlowId(5) }),
            Ok(DispatchOutcome::FlowOpened)
        );
    }

    #[test]
    fn dispatcher_applies_data_finish_reset_and_close_rules() {
        let authenticator = AttachmentAuthenticator::new(b"secret").expect("secret");
        let session_id = SessionId([9; 16]);
        let mut session = tracer();
        session
            .bind_negotiated_parameters(parameters())
            .expect("bind parameters");
        let mut dispatcher = SessionFrameDispatcher::new(session_id, CarrierId(4), &authenticator);
        dispatcher
            .dispatch(&mut session, attach_frame(&authenticator, session_id))
            .expect("attach");
        dispatcher
            .dispatch(&mut session, Frame::StreamOpen { flow_id: FlowId(5) })
            .expect("open");
        assert_eq!(
            dispatcher.dispatch(
                &mut session,
                Frame::StreamData {
                    flow_id: FlowId(5),
                    epoch: Epoch(0),
                    sequence: Sequence(0),
                    bytes: vec![7],
                },
            ),
            Ok(DispatchOutcome::Delivered(vec![7]))
        );
        assert_eq!(
            dispatcher.dispatch(
                &mut session,
                Frame::StreamFinish {
                    flow_id: FlowId(5),
                    epoch: Epoch(0),
                    final_next_sequence: Sequence(1),
                },
            ),
            Ok(DispatchOutcome::FlowFinished)
        );
        assert_eq!(
            dispatcher.dispatch(
                &mut session,
                Frame::StreamData {
                    flow_id: FlowId(5),
                    epoch: Epoch(0),
                    sequence: Sequence(1),
                    bytes: vec![8],
                },
            ),
            Err(DispatchError::Trace(TraceError::FlowFinished))
        );
        assert_eq!(
            dispatcher.dispatch(
                &mut session,
                Frame::StreamReset {
                    flow_id: FlowId(5),
                    error: ProtocolErrorCode::FlowState,
                },
            ),
            Ok(DispatchOutcome::FlowReset {
                error: ProtocolErrorCode::FlowState
            })
        );
        assert_eq!(
            dispatcher.dispatch(
                &mut session,
                Frame::ConnectionClose {
                    error: ProtocolErrorCode::Internal,
                },
            ),
            Ok(DispatchOutcome::ConnectionClosed {
                error: ProtocolErrorCode::Internal
            })
        );
        assert!(dispatcher.is_closed());
    }

    #[test]
    fn retiring_carrier_carries_only_in_flight_frames() {
        let authenticator = AttachmentAuthenticator::new(b"secret").expect("secret");
        let session_id = SessionId([9; 16]);
        let mut session = tracer();
        session
            .bind_negotiated_parameters(parameters())
            .expect("bind parameters");
        let mut old = SessionFrameDispatcher::new(session_id, CarrierId(4), &authenticator);
        old.dispatch(&mut session, attach_frame(&authenticator, session_id))
            .expect("old attachment");
        old.dispatch(&mut session, Frame::StreamOpen { flow_id: FlowId(5) })
            .expect("open");
        assert_eq!(session.begin_transition(), Ok(Epoch(1)));

        let mut replacement = SessionFrameDispatcher::new(session_id, CarrierId(6), &authenticator);
        replacement
            .dispatch(
                &mut session,
                attach_frame_at(&authenticator, session_id, Epoch(1), [4; 16]),
            )
            .expect("replacement attachment");
        assert_eq!(
            old.dispatch(
                &mut session,
                Frame::StreamData {
                    flow_id: FlowId(5),
                    epoch: Epoch(0),
                    sequence: Sequence(0),
                    bytes: vec![7],
                },
            ),
            Ok(DispatchOutcome::Delivered(vec![7]))
        );
        assert_eq!(
            old.dispatch(&mut session, Frame::StreamOpen { flow_id: FlowId(6) }),
            Err(DispatchError::CarrierNotAttached)
        );
        session.complete_transition();
        assert_eq!(
            old.dispatch(
                &mut session,
                Frame::StreamData {
                    flow_id: FlowId(5),
                    epoch: Epoch(0),
                    sequence: Sequence(1),
                    bytes: vec![8],
                },
            ),
            Err(DispatchError::CarrierNotAttached)
        );
    }
}

//! Redacted event vocabulary; storage and export are application concerns.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SessionEvent {
    FlowOpened,
    FlowTimedOut,
    PendingLimitReached,
    DuplicateIgnored,
    OutOfOrderRejected,
    TransitionStarted {
        from: CarrierClass,
        to: CarrierClass,
        reason: TransitionReason,
    },
    TransitionCompleted,
    TransitionRejected {
        reason: TransitionRejection,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CarrierClass {
    Quic,
    Tls,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TransitionReason {
    PrimaryUnhealthy,
    PrimaryRecovered,
    LossThresholdExceeded,
    LatencyThresholdExceeded,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TransitionRejection {
    IncompatibleVersion,
    DatagramFlow,
    FlowControlExhausted,
    AttachmentRejected,
}

/// Payload-free Stage 2 operational event vocabulary.
///
/// Values deliberately exclude credentials, destination addresses, stream
/// contents, and transport error text so callers can safely export them.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum QuicRelayEvent {
    ConnectionAccepted,
    AuthenticationRejected,
    DestinationRejected,
    SessionQuotaRejected,
    FlowQuotaRejected,
    ConnectFailed,
    FlowRelayed,
    ShutdownStarted,
}

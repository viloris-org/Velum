//! Redacted event vocabulary; storage and export are application concerns.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SessionEvent {
    FlowOpened,
    FlowTimedOut,
    PendingLimitReached,
    DuplicateIgnored,
    OutOfOrderRejected,
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

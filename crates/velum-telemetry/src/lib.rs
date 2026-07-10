//! Redacted event vocabulary; storage and export are application concerns.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SessionEvent {
    FlowOpened,
    FlowTimedOut,
    PendingLimitReached,
    DuplicateIgnored,
    OutOfOrderRejected,
}

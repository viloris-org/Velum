//! Narrow carrier-facing types. Implementations are deferred to Stage 2.

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct CarrierId(pub u64);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Reliability {
    Stream,
    Datagram,
}

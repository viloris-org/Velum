//! Carrier contract shared by session state and transport implementations.
//!
//! Carriers expose transport primitives and measurements only. Logical flow
//! identifiers, epochs, acknowledgements, and placement remain session-owned.

#![allow(async_fn_in_trait)]

use velum_protocol::{Epoch, FlowId};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct CarrierId(pub u64);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CarrierKind {
    Quic,
    Tls,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Reliability {
    Stream,
    Datagram,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CarrierCapabilities {
    pub streams: bool,
    pub datagrams: bool,
    pub max_datagram_payload: Option<usize>,
}

impl CarrierCapabilities {
    pub const fn supports(self, reliability: Reliability) -> bool {
        match reliability {
            Reliability::Stream => self.streams,
            Reliability::Datagram => self.datagrams,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CarrierHealth {
    pub round_trip_time_millis: Option<u64>,
    pub loss_parts_per_million: Option<u32>,
    pub is_healthy: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CarrierError {
    Closed,
    UnsupportedReliability(Reliability),
    DatagramTooLarge { maximum: usize, actual: usize },
    Transport,
}

/// Metadata that the session supplies when opening a carrier stream.
///
/// It is deliberately not acknowledgement state: transport completion is
/// evidence for the session, never the session's delivery truth.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StreamRequest {
    pub flow_id: FlowId,
    pub epoch: Epoch,
}

/// Narrow asynchronous carrier boundary.
///
/// The associated stream is transport-specific so this crate does not impose a
/// byte framing format before the protocol layer defines one.
pub trait Carrier: Send + Sync {
    type ReliableStream: Send;

    fn id(&self) -> CarrierId;
    fn kind(&self) -> CarrierKind;
    fn capabilities(&self) -> CarrierCapabilities;
    fn health(&self) -> CarrierHealth;

    async fn open_reliable_stream(
        &self,
        request: StreamRequest,
    ) -> Result<Self::ReliableStream, CarrierError>;
    async fn accept_reliable_stream(&self) -> Result<Self::ReliableStream, CarrierError>;
    async fn send_datagram(&self, bytes: &[u8]) -> Result<(), CarrierError>;
    async fn receive_datagram(&self) -> Result<Vec<u8>, CarrierError>;
    fn close(&self);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capability_checks_do_not_conflate_datagrams_and_streams() {
        let capabilities = CarrierCapabilities {
            streams: true,
            datagrams: false,
            max_datagram_payload: None,
        };

        assert!(capabilities.supports(Reliability::Stream));
        assert!(!capabilities.supports(Reliability::Datagram));
    }
}

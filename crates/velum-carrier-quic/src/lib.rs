//! QUIC carrier implementation behind the carrier contract.
//!
//! This crate intentionally maps only QUIC streams and datagrams. Session frame
//! encoding and authentication remain future protocol and server work.

use velum_carrier_api::{
    Carrier, CarrierCapabilities, CarrierError, CarrierHealth, CarrierId, CarrierKind,
    StreamRequest,
};

/// One bidirectional QUIC stream opened for a session-owned logical flow.
pub struct QuicStream {
    send: quinn::SendStream,
    receive: quinn::RecvStream,
}

impl QuicStream {
    pub async fn write_all(&mut self, bytes: &[u8]) -> Result<(), CarrierError> {
        self.send
            .write_all(bytes)
            .await
            .map_err(|_| CarrierError::Transport)
    }

    pub async fn read_chunk(&mut self, maximum: usize) -> Result<Option<Vec<u8>>, CarrierError> {
        self.receive
            .read_chunk(maximum, true)
            .await
            .map(|chunk| chunk.map(|chunk| chunk.bytes.to_vec()))
            .map_err(|_| CarrierError::Transport)
    }

    pub fn finish(&mut self) -> Result<(), CarrierError> {
        self.send.finish().map_err(|_| CarrierError::Transport)
    }
}

/// A single authenticated QUIC connection. It owns no session delivery state.
#[derive(Clone)]
pub struct QuicCarrier {
    id: CarrierId,
    connection: quinn::Connection,
}

impl QuicCarrier {
    pub fn new(id: CarrierId, connection: quinn::Connection) -> Self {
        Self { id, connection }
    }

    fn datagram_limit(&self) -> Result<usize, CarrierError> {
        self.connection
            .max_datagram_size()
            .ok_or(CarrierError::UnsupportedReliability(
                velum_carrier_api::Reliability::Datagram,
            ))
    }
}

impl Carrier for QuicCarrier {
    type ReliableStream = QuicStream;

    fn id(&self) -> CarrierId {
        self.id
    }

    fn kind(&self) -> CarrierKind {
        CarrierKind::Quic
    }

    fn capabilities(&self) -> CarrierCapabilities {
        let max_datagram_payload = self.connection.max_datagram_size();
        CarrierCapabilities {
            streams: true,
            datagrams: max_datagram_payload.is_some(),
            max_datagram_payload,
        }
    }

    fn health(&self) -> CarrierHealth {
        CarrierHealth {
            round_trip_time_millis: None,
            loss_parts_per_million: None,
            is_healthy: self.connection.close_reason().is_none(),
        }
    }

    async fn open_reliable_stream(
        &self,
        _request: StreamRequest,
    ) -> Result<Self::ReliableStream, CarrierError> {
        self.connection
            .open_bi()
            .await
            .map(|(send, receive)| QuicStream { send, receive })
            .map_err(|_| CarrierError::Closed)
    }

    async fn accept_reliable_stream(&self) -> Result<Self::ReliableStream, CarrierError> {
        self.connection
            .accept_bi()
            .await
            .map(|(send, receive)| QuicStream { send, receive })
            .map_err(|_| CarrierError::Closed)
    }

    async fn send_datagram(&self, bytes: &[u8]) -> Result<(), CarrierError> {
        let maximum = self.datagram_limit()?;
        if bytes.len() > maximum {
            return Err(CarrierError::DatagramTooLarge {
                maximum,
                actual: bytes.len(),
            });
        }
        self.connection
            .send_datagram(bytes.to_vec().into())
            .map_err(|_| CarrierError::Transport)
    }

    async fn receive_datagram(&self) -> Result<Vec<u8>, CarrierError> {
        self.connection
            .read_datagram()
            .await
            .map(|bytes| bytes.to_vec())
            .map_err(|_| CarrierError::Closed)
    }

    fn close(&self) {
        self.connection.close(0_u32.into(), b"velum shutdown");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn requires_carrier_contract<T: Carrier<ReliableStream = QuicStream>>() {}

    #[test]
    fn quic_carrier_implements_the_public_contract() {
        requires_carrier_contract::<QuicCarrier>();
    }
}

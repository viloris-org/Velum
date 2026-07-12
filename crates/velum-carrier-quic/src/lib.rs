//! QUIC carrier implementation behind the carrier contract.
//!
//! This crate intentionally maps only QUIC streams and datagrams. Session frame
//! encoding and authentication remain future protocol and server work.

use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use velum_carrier_api::{
    Carrier, CarrierCapabilities, CarrierError, CarrierHealth, CarrierKind, StreamRequest,
};

pub use velum_carrier_api::CarrierId;

/// Bounded QUIC transport settings shared by the client and listener layers.
///
/// These limits are deliberately separate from logical-flow policy. The values
/// bound peer-controlled QUIC state before a stream reaches application code.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QuicTransportProfile {
    pub max_bidirectional_streams: u32,
    pub stream_receive_window: u32,
    pub connection_receive_window: u32,
    pub send_window: u64,
    pub idle_timeout: Duration,
    pub keep_alive_interval: Option<Duration>,
    pub datagram_receive_buffer_bytes: usize,
    pub datagram_send_buffer_bytes: usize,
}

impl Default for QuicTransportProfile {
    fn default() -> Self {
        Self {
            max_bidirectional_streams: 64,
            stream_receive_window: 256 * 1024,
            connection_receive_window: 4 * 1024 * 1024,
            send_window: 4 * 1024 * 1024,
            idle_timeout: Duration::from_secs(30),
            keep_alive_interval: None,
            datagram_receive_buffer_bytes: 256 * 1024,
            datagram_send_buffer_bytes: 256 * 1024,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum QuicTransportProfileError {
    ZeroStreamLimit,
    ZeroStreamWindow,
    ConnectionWindowTooSmall,
    ZeroSendWindow,
    ZeroIdleTimeout,
    KeepAliveNotShorterThanIdleTimeout,
    ZeroDatagramBuffer,
    ValueOutOfRange,
}

impl QuicTransportProfile {
    pub fn validate(&self) -> Result<(), QuicTransportProfileError> {
        if self.max_bidirectional_streams == 0 {
            return Err(QuicTransportProfileError::ZeroStreamLimit);
        }
        if self.stream_receive_window == 0 {
            return Err(QuicTransportProfileError::ZeroStreamWindow);
        }
        if self.connection_receive_window < self.stream_receive_window {
            return Err(QuicTransportProfileError::ConnectionWindowTooSmall);
        }
        if self.send_window == 0 {
            return Err(QuicTransportProfileError::ZeroSendWindow);
        }
        if self.idle_timeout.is_zero() {
            return Err(QuicTransportProfileError::ZeroIdleTimeout);
        }
        if self
            .keep_alive_interval
            .is_some_and(|interval| interval.is_zero() || interval >= self.idle_timeout)
        {
            return Err(QuicTransportProfileError::KeepAliveNotShorterThanIdleTimeout);
        }
        if self.datagram_receive_buffer_bytes == 0 || self.datagram_send_buffer_bytes == 0 {
            return Err(QuicTransportProfileError::ZeroDatagramBuffer);
        }
        Ok(())
    }

    /// Builds the Quinn configuration applied during the TLS handshake.
    pub fn build(&self) -> Result<Arc<quinn::TransportConfig>, QuicTransportProfileError> {
        self.validate()?;
        let max_streams = self.max_bidirectional_streams.into();
        let stream_window = self.stream_receive_window.into();
        let connection_window = self.connection_receive_window.into();
        let idle_timeout = self
            .idle_timeout
            .try_into()
            .map_err(|_| QuicTransportProfileError::ValueOutOfRange)?;

        let mut transport = quinn::TransportConfig::default();
        transport
            .max_concurrent_bidi_streams(max_streams)
            .max_concurrent_uni_streams(0_u32.into())
            .stream_receive_window(stream_window)
            .receive_window(connection_window)
            .send_window(self.send_window)
            .max_idle_timeout(Some(idle_timeout))
            .keep_alive_interval(self.keep_alive_interval)
            .datagram_receive_buffer_size(Some(self.datagram_receive_buffer_bytes))
            .datagram_send_buffer_size(self.datagram_send_buffer_bytes);
        Ok(Arc::new(transport))
    }
}

/// One bidirectional QUIC stream opened for a session-owned logical flow.
pub struct QuicStream {
    send: quinn::SendStream,
    receive: quinn::RecvStream,
}

impl QuicStream {
    /// Transfers the QUIC stream halves to an application-owned relay.
    ///
    /// The carrier remains responsible for transport mapping; the application
    /// owns local socket lifecycle and destination policy enforcement.
    pub fn into_parts(self) -> (quinn::SendStream, quinn::RecvStream) {
        (self.send, self.receive)
    }

    /// Explicitly terminates a stream that reached application admission but
    /// cannot receive a flow slot.
    pub fn reject(mut self) {
        let _ = self.send.reset(0_u32.into());
        let _ = self.receive.stop(0_u32.into());
    }

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
    health_counters: Arc<Mutex<HealthCounters>>,
}

impl QuicCarrier {
    pub fn new(id: CarrierId, connection: quinn::Connection) -> Self {
        Self {
            id,
            connection,
            health_counters: Arc::new(Mutex::new(HealthCounters::default())),
        }
    }

    /// Accepts an incoming bidirectional stream without exposing Quinn to the
    /// application listener's carrier dispatch loop.
    pub async fn accept_stream(&self) -> Result<QuicStream, CarrierError> {
        self.connection
            .accept_bi()
            .await
            .map(|(send, receive)| QuicStream { send, receive })
            .map_err(|_| CarrierError::Closed)
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
        let stats = self.connection.stats();
        CarrierHealth {
            round_trip_time_millis: Some(
                stats.path.rtt.as_millis().min(u128::from(u64::MAX)) as u64
            ),
            loss_parts_per_million: self.sample_loss(&stats),
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
        self.accept_stream().await
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

#[derive(Default)]
struct HealthCounters {
    sent_packets: u64,
    lost_packets: u64,
    sent_plpmtud_probes: u64,
}

impl HealthCounters {
    fn observe(
        &mut self,
        sent_packets: u64,
        lost_packets: u64,
        sent_plpmtud_probes: u64,
    ) -> Option<u32> {
        let application_packets = sent_packets.saturating_sub(sent_plpmtud_probes);
        let prior_application_packets = self.sent_packets.saturating_sub(self.sent_plpmtud_probes);
        let loss = loss_parts_per_million(
            lost_packets.saturating_sub(self.lost_packets),
            application_packets.saturating_sub(prior_application_packets),
        );
        self.sent_packets = sent_packets;
        self.lost_packets = lost_packets;
        self.sent_plpmtud_probes = sent_plpmtud_probes;
        loss
    }
}

impl QuicCarrier {
    fn sample_loss(&self, stats: &quinn::ConnectionStats) -> Option<u32> {
        self.health_counters.lock().ok().and_then(|mut counters| {
            counters.observe(
                stats.path.sent_packets,
                stats.path.lost_packets,
                stats.path.sent_plpmtud_probes,
            )
        })
    }
}

fn loss_parts_per_million(lost_packets: u64, sent_packets: u64) -> Option<u32> {
    if sent_packets == 0 {
        return None;
    }
    Some(
        lost_packets
            .saturating_mul(1_000_000)
            .saturating_div(sent_packets)
            .min(u64::from(u32::MAX)) as u32,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn requires_carrier_contract<T: Carrier<ReliableStream = QuicStream>>() {}

    #[test]
    fn quic_carrier_implements_the_public_contract() {
        requires_carrier_contract::<QuicCarrier>();
    }

    #[test]
    fn transport_profile_rejects_unbounded_or_inconsistent_values() {
        let mut profile = QuicTransportProfile::default();
        assert!(profile.validate().is_ok());
        profile.max_bidirectional_streams = 0;
        assert_eq!(
            profile.validate(),
            Err(QuicTransportProfileError::ZeroStreamLimit)
        );
        profile.max_bidirectional_streams = 1;
        profile.connection_receive_window = profile.stream_receive_window - 1;
        assert_eq!(
            profile.validate(),
            Err(QuicTransportProfileError::ConnectionWindowTooSmall)
        );
    }

    #[test]
    fn loss_measurement_is_bounded_and_handles_an_idle_path() {
        assert_eq!(loss_parts_per_million(0, 0), None);
        assert_eq!(loss_parts_per_million(1, 4), Some(250_000));
        assert_eq!(loss_parts_per_million(u64::MAX, 1), Some(u32::MAX));
    }

    #[test]
    fn health_sampling_uses_only_the_latest_transport_interval() {
        let mut counters = HealthCounters::default();
        assert_eq!(counters.observe(10, 2, 1), Some(222_222));
        assert_eq!(counters.observe(20, 3, 1), Some(100_000));
        assert_eq!(counters.observe(20, 3, 1), None);
    }
}

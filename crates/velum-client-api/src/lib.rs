//! Direct, in-process client API for the experimental Stage 2 QUIC relay.
//!
//! This crate deliberately owns no local proxy listener. Callers open and use
//! a [`ClientStream`] directly. The remote control record remains the
//! application-owned Stage 2 format and is not a v0 interoperability claim.

use std::{net::SocketAddr, sync::Arc, time::Duration};

use rustls::{RootCertStore, pki_types::CertificateDer};
use tokio::time::timeout;
use velum_carrier_quic::QuicTransportProfile;
use velum_protocol::{Datagram, DatagramSessionId};

const MAX_CREDENTIAL_BYTES: usize = 128;
const OPEN_HEADER_BYTES: usize = 4;

/// The version of the direct in-process API.
pub const API_VERSION: u16 = 1;

/// Immutable inputs for one authenticated client connection.
pub struct ClientConfig {
    relay_address: SocketAddr,
    server_name: String,
    credential: Arc<[u8]>,
    root_certificates: Vec<CertificateDer<'static>>,
    connect_timeout: Duration,
}

impl ClientConfig {
    /// Builds configuration without exposing credential bytes through errors.
    pub fn new(
        relay_address: SocketAddr,
        server_name: String,
        credential: Vec<u8>,
        root_certificates: Vec<CertificateDer<'static>>,
        connect_timeout: Duration,
    ) -> Result<Self, ClientConfigError> {
        if server_name.is_empty() {
            return Err(ClientConfigError::EmptyServerName);
        }
        if credential.is_empty() || credential.len() > MAX_CREDENTIAL_BYTES {
            return Err(ClientConfigError::InvalidCredentialLength);
        }
        if root_certificates.is_empty() {
            return Err(ClientConfigError::MissingRootCertificate);
        }
        if connect_timeout.is_zero() {
            return Err(ClientConfigError::ZeroConnectTimeout);
        }
        Ok(Self {
            relay_address,
            server_name,
            credential: credential.into(),
            root_certificates,
            connect_timeout,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClientConfigError {
    EmptyServerName,
    InvalidCredentialLength,
    MissingRootCertificate,
    ZeroConnectTimeout,
}

/// Stable, payload-free direct-client failure categories.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClientError {
    Certificate,
    ConnectTimeout,
    Connection,
    ControlTooLarge,
    DatagramTooLarge,
    DatagramUnavailable,
    Protocol,
    Transport,
}

/// One authenticated QUIC client connection.
pub struct Client {
    _endpoint: quinn::Endpoint,
    connection: quinn::Connection,
    credential: Arc<[u8]>,
}

impl Client {
    /// Establishes the QUIC connection with the configured certificate roots.
    pub async fn connect(config: ClientConfig) -> Result<Self, ClientError> {
        let mut roots = RootCertStore::empty();
        for certificate in config.root_certificates {
            roots
                .add(certificate)
                .map_err(|_| ClientError::Certificate)?;
        }
        let mut client_config = quinn::ClientConfig::with_root_certificates(Arc::new(roots))
            .map_err(|_| ClientError::Certificate)?;
        let transport = QuicTransportProfile {
            keep_alive_interval: Some(Duration::from_secs(10)),
            ..Default::default()
        }
        .build()
        .map_err(|_| ClientError::Transport)?;
        client_config.transport_config(transport);
        let mut endpoint = quinn::Endpoint::client("0.0.0.0:0".parse().expect("valid address"))
            .map_err(|_| ClientError::Connection)?;
        endpoint.set_default_client_config(client_config);
        let connecting = endpoint
            .connect(config.relay_address, &config.server_name)
            .map_err(|_| ClientError::Connection)?;
        let connection = timeout(config.connect_timeout, connecting)
            .await
            .map_err(|_| ClientError::ConnectTimeout)?
            .map_err(|_| ClientError::Connection)?;
        Ok(Self {
            _endpoint: endpoint,
            connection,
            credential: config.credential,
        })
    }

    /// Opens one direct reliable stream to an exact server-authorized target.
    pub async fn open_stream(&self, target: SocketAddr) -> Result<ClientStream, ClientError> {
        let (mut send, receive) = self
            .connection
            .open_bi()
            .await
            .map_err(|_| ClientError::Transport)?;
        let control = encode_open(&self.credential, target)?;
        send.write_all(&control)
            .await
            .map_err(|_| ClientError::Transport)?;
        Ok(ClientStream { send, receive })
    }

    /// Sends one bounded, explicitly unreliable datagram to an exact target.
    pub fn send_datagram(
        &self,
        session_id: DatagramSessionId,
        destination: SocketAddr,
        payload: &[u8],
    ) -> Result<(), ClientError> {
        let encoded = Datagram::ClientToServer {
            credential: self.credential.to_vec(),
            session_id,
            destination,
            payload: payload.to_vec(),
        }
        .encode()
        .map_err(|_| ClientError::DatagramTooLarge)?;
        let maximum = self
            .connection
            .max_datagram_size()
            .ok_or(ClientError::DatagramUnavailable)?;
        if encoded.len() > maximum {
            return Err(ClientError::DatagramTooLarge);
        }
        self.connection
            .send_datagram(encoded.into())
            .map_err(|_| ClientError::Transport)
    }

    /// Receives one server-to-client datagram, rejecting malformed directions.
    pub async fn receive_datagram(&self) -> Result<ClientDatagram, ClientError> {
        let encoded = self
            .connection
            .read_datagram()
            .await
            .map_err(|_| ClientError::Transport)?;
        match Datagram::decode(&encoded).map_err(|_| ClientError::Protocol)? {
            Datagram::ServerToClient {
                session_id,
                source,
                payload,
            } => Ok(ClientDatagram {
                session_id,
                source,
                payload,
            }),
            Datagram::ClientToServer { .. } => Err(ClientError::Protocol),
        }
    }

    /// Closes the connection and all streams owned by this client.
    pub fn close(&self) {
        self.connection.close(0_u32.into(), b"velum client closed");
    }
}

/// A caller-owned reliable stream with explicit backpressure at every I/O call.
pub struct ClientStream {
    send: quinn::SendStream,
    receive: quinn::RecvStream,
}

/// An authenticated response received over the native unreliable datagram path.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClientDatagram {
    pub session_id: DatagramSessionId,
    pub source: SocketAddr,
    pub payload: Vec<u8>,
}

impl ClientStream {
    pub async fn write_all(&mut self, bytes: &[u8]) -> Result<(), ClientError> {
        self.send
            .write_all(bytes)
            .await
            .map_err(|_| ClientError::Transport)
    }

    pub async fn read(&mut self, bytes: &mut [u8]) -> Result<Option<usize>, ClientError> {
        self.receive
            .read(bytes)
            .await
            .map_err(|_| ClientError::Transport)
    }

    pub fn finish(&mut self) -> Result<(), ClientError> {
        self.send.finish().map_err(|_| ClientError::Transport)
    }
}

fn encode_open(credential: &[u8], target: SocketAddr) -> Result<Vec<u8>, ClientError> {
    let credential_length =
        u16::try_from(credential.len()).map_err(|_| ClientError::ControlTooLarge)?;
    let target = target.to_string();
    let target_length = u16::try_from(target.len()).map_err(|_| ClientError::ControlTooLarge)?;
    let capacity = OPEN_HEADER_BYTES
        .checked_add(credential.len())
        .and_then(|value| value.checked_add(target.len()))
        .ok_or(ClientError::ControlTooLarge)?;
    let mut encoded = Vec::with_capacity(capacity);
    encoded.extend_from_slice(&credential_length.to_be_bytes());
    encoded.extend_from_slice(&target_length.to_be_bytes());
    encoded.extend_from_slice(credential);
    encoded.extend_from_slice(target.as_bytes());
    Ok(encoded)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn configuration_rejects_unbounded_or_empty_security_inputs() {
        let certificate = CertificateDer::from(vec![1, 2, 3]);
        let relay = "192.0.2.1:443".parse().expect("address");
        assert!(matches!(
            ClientConfig::new(
                relay,
                String::new(),
                vec![7],
                vec![certificate.clone()],
                Duration::from_secs(1),
            ),
            Err(ClientConfigError::EmptyServerName)
        ));
        assert!(matches!(
            ClientConfig::new(
                relay,
                "relay.example".into(),
                vec![7; 129],
                vec![certificate],
                Duration::from_secs(1),
            ),
            Err(ClientConfigError::InvalidCredentialLength)
        ));
    }

    #[test]
    fn open_record_preserves_exact_target_and_credential_bounds() {
        let encoded =
            encode_open(&[7, 8], "192.0.2.10:443".parse().expect("address")).expect("open record");
        assert_eq!(&encoded[..4], &[0, 2, 0, 14]);
        assert_eq!(&encoded[4..6], &[7, 8]);
        assert_eq!(&encoded[6..], b"192.0.2.10:443");
    }
}

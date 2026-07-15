//! Experimental application-owned control record for the QUIC slice.
//!
//! This record is not the Velum v0 wire protocol and must be replaced before
//! this listener claims v0 interoperability.

use std::{net::SocketAddr, sync::Arc, time::Duration};

use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
    sync::Mutex,
    time::timeout,
};
use velum_carrier_quic::QuicStream;
use velum_server::{
    AdmissionControl, AdmissionError, Authenticator, DestinationPolicy, PrincipalId, SessionLease,
};
use velum_telemetry::QuicRelayEvent;

const MAX_SECRET_BYTES: usize = 128;
const HEADER_BYTES: usize = 4;
const RELAY_COPY_BUFFER_BYTES: usize = 16 * 1024;

pub mod acme;
pub mod admin;
pub mod cli;
pub mod config;
mod cover_listener;
mod datagram;
pub mod deployment;
pub mod enrollment;
mod listener;
mod setup;
mod terminal;
mod uninstall;

pub use cover_listener::{bind_cover_listener, serve_cover_listener};
pub use listener::{bind_quic_listener, serve_quic_listener};

/// Versioned, application-owned configuration for the experimental QUIC
/// listener. Secrets and certificates are supplied out of band.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QuicRelayConfig {
    pub schema_version: u16,
    pub connect_timeout: Duration,
    pub control_timeout: Duration,
    pub shutdown_timeout: Duration,
    pub max_control_bytes: usize,
    pub max_connections: usize,
    pub max_streams_per_connection: usize,
}

impl Default for QuicRelayConfig {
    fn default() -> Self {
        Self {
            schema_version: 1,
            connect_timeout: Duration::from_secs(5),
            control_timeout: Duration::from_secs(5),
            shutdown_timeout: Duration::from_secs(5),
            max_control_bytes: MAX_SECRET_BYTES + 64,
            max_connections: 1_024,
            max_streams_per_connection: 64,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConfigError {
    UnsupportedSchema,
    ZeroConnectTimeout,
    ZeroControlTimeout,
    ZeroShutdownTimeout,
    ControlLimitTooSmall,
    ZeroConnectionLimit,
    ZeroStreamLimit,
    StreamLimitTooLarge,
}

impl QuicRelayConfig {
    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.schema_version != 1 {
            return Err(ConfigError::UnsupportedSchema);
        }
        if self.connect_timeout.is_zero() {
            return Err(ConfigError::ZeroConnectTimeout);
        }
        if self.control_timeout.is_zero() {
            return Err(ConfigError::ZeroControlTimeout);
        }
        if self.shutdown_timeout.is_zero() {
            return Err(ConfigError::ZeroShutdownTimeout);
        }
        if self.max_control_bytes < HEADER_BYTES + 1 + "[::1]:65535".len() {
            return Err(ConfigError::ControlLimitTooSmall);
        }
        if self.max_connections == 0 {
            return Err(ConfigError::ZeroConnectionLimit);
        }
        if self.max_streams_per_connection == 0 {
            return Err(ConfigError::ZeroStreamLimit);
        }
        Ok(())
    }

    /// Converts application flow limits into the QUIC transport limits that
    /// constrain a peer before listener dispatch runs.
    pub fn transport_profile(
        &self,
    ) -> Result<velum_carrier_quic::QuicTransportProfile, ConfigError> {
        Ok(velum_carrier_quic::QuicTransportProfile {
            max_bidirectional_streams: u32::try_from(self.max_streams_per_connection)
                .map_err(|_| ConfigError::StreamLimitTooLarge)?,
            ..Default::default()
        })
    }
}

/// Export boundary for payload-free lifecycle events.
pub trait RelayObserver: Send + Sync {
    fn record(&self, event: QuicRelayEvent);
}

#[derive(Default)]
pub struct NoopRelayObserver;

impl RelayObserver for NoopRelayObserver {
    fn record(&self, _: QuicRelayEvent) {}
}

/// Shared process-local state for a single admitted QUIC connection.
#[derive(Clone)]
pub struct RelayAdmission {
    pub authenticator: Arc<Authenticator>,
    pub destinations: Arc<DestinationPolicy>,
    pub quotas: Arc<Mutex<AdmissionControl>>,
}

#[derive(Clone, Copy)]
struct AdmittedSession {
    principal: PrincipalId,
    lease: SessionLease,
}

/// Admission state shared by all streams on one QUIC connection.
///
/// Each stream still authenticates its control record, but successful streams
/// must resolve to the same principal and consume one connection-owned session
/// lease. This keeps session and flow quotas semantically distinct.
#[derive(Clone, Default)]
pub struct ConnectionAdmission {
    session: Arc<Mutex<Option<AdmittedSession>>>,
}

impl ConnectionAdmission {
    pub async fn is_authenticated(&self) -> bool {
        self.session.lock().await.is_some()
    }

    /// Authenticates one native datagram and binds this connection to its
    /// principal before an association can be created. Datagram traffic does
    /// not consume a reliable-flow quota.
    pub(crate) async fn admit_datagram(
        &self,
        admission: &RelayAdmission,
        credential: &[u8],
        observer: &dyn RelayObserver,
    ) -> Result<(), RelayError> {
        let principal = admission
            .authenticator
            .authenticate(credential)
            .map_err(|_| {
                observer.record(QuicRelayEvent::AuthenticationRejected);
                RelayError::AuthenticationRejected
            })?;
        let mut session = self.session.lock().await;
        match *session {
            Some(existing) if existing.principal != principal => {
                observer.record(QuicRelayEvent::AuthenticationRejected);
                Err(RelayError::AuthenticationRejected)
            }
            Some(_) => Ok(()),
            None => {
                let lease = open_session(&admission.quotas, principal, observer).await?;
                *session = Some(AdmittedSession { principal, lease });
                Ok(())
            }
        }
    }

    async fn reserve_flow(
        &self,
        admission: &RelayAdmission,
        principal: PrincipalId,
        observer: &dyn RelayObserver,
    ) -> Result<SessionLease, RelayError> {
        let mut session = self.session.lock().await;
        let (lease, created_session) = match *session {
            Some(existing) if existing.principal != principal => {
                observer.record(QuicRelayEvent::AuthenticationRejected);
                return Err(RelayError::AuthenticationRejected);
            }
            Some(existing) => (existing.lease, false),
            None => {
                let lease = open_session(&admission.quotas, principal, observer).await?;
                *session = Some(AdmittedSession { principal, lease });
                (lease, true)
            }
        };

        if let Err(error) = open_flow(&admission.quotas, principal, lease, observer).await {
            // A failed first flow must not leave a session quota consumed.
            if created_session {
                let _ = admission
                    .quotas
                    .lock()
                    .await
                    .close_session(principal, lease);
                *session = None;
            }
            return Err(error);
        }
        Ok(lease)
    }

    async fn release_flow(&self, admission: &RelayAdmission, lease: SessionLease) {
        let session = self.session.lock().await;
        if let Some(session) = *session
            && session.lease == lease
        {
            let _ = admission
                .quotas
                .lock()
                .await
                .close_flow(session.principal, lease);
        }
    }

    pub async fn close(&self, admission: &RelayAdmission) {
        let mut session = self.session.lock().await;
        if let Some(session_lease) = session.take() {
            let _ = admission
                .quotas
                .lock()
                .await
                .close_session(session_lease.principal, session_lease.lease);
        }
    }
}

/// Failure classification intentionally omits credentials and destination data.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RelayError {
    InvalidControl,
    AuthenticationRejected,
    DestinationRejected,
    SessionQuotaRejected,
    FlowQuotaRejected,
    ConnectFailed,
    Transport,
}

/// Accepts one authenticated QUIC flow, connects only to an exact allowed
/// target, and relays bytes bidirectionally. Its flow lease is released on
/// every return path; the connection releases the shared session lease.
pub async fn relay_quic_stream(
    stream: QuicStream,
    admission: &RelayAdmission,
    connection: &ConnectionAdmission,
    config: &QuicRelayConfig,
    observer: &dyn RelayObserver,
) -> Result<(), RelayError> {
    config.validate().map_err(|_| RelayError::InvalidControl)?;
    let (mut send, mut receive) = stream.into_parts();
    let request = timeout(
        config.control_timeout,
        read_open_request(&mut receive, config.max_control_bytes),
    )
    .await
    .map_err(|_| {
        observer.record(QuicRelayEvent::ControlRejected);
        RelayError::InvalidControl
    })?
    .inspect_err(|_| {
        observer.record(QuicRelayEvent::ControlRejected);
    })?;
    let principal = admission
        .authenticator
        .authenticate(&request.secret)
        .map_err(|_| {
            observer.record(QuicRelayEvent::AuthenticationRejected);
            RelayError::AuthenticationRejected
        })?;
    if !admission.destinations.allows(request.target) {
        observer.record(QuicRelayEvent::DestinationRejected);
        return Err(RelayError::DestinationRejected);
    }

    let lease = connection
        .reserve_flow(admission, principal, observer)
        .await?;
    let result = relay_parts(
        &mut send,
        &mut receive,
        request.target,
        config.connect_timeout,
    )
    .await;
    connection.release_flow(admission, lease).await;
    if result.is_ok() {
        observer.record(QuicRelayEvent::FlowRelayed);
    } else {
        observer.record(QuicRelayEvent::ConnectFailed);
    }
    result
}

async fn read_open_request(
    receive: &mut quinn::RecvStream,
    maximum: usize,
) -> Result<OpenRequest, RelayError> {
    if maximum < HEADER_BYTES {
        return Err(RelayError::InvalidControl);
    }
    let mut header = [0; HEADER_BYTES];
    receive
        .read_exact(&mut header)
        .await
        .map_err(|_| RelayError::Transport)?;
    let secret_length = usize::from(u16::from_be_bytes([header[0], header[1]]));
    let target_length = usize::from(u16::from_be_bytes([header[2], header[3]]));
    let length = HEADER_BYTES
        .checked_add(secret_length)
        .and_then(|value| value.checked_add(target_length))
        .ok_or(RelayError::InvalidControl)?;
    if length > maximum {
        return Err(RelayError::InvalidControl);
    }
    let mut encoded = vec![0; length];
    encoded[..HEADER_BYTES].copy_from_slice(&header);
    receive
        .read_exact(&mut encoded[HEADER_BYTES..])
        .await
        .map_err(|_| RelayError::Transport)?;
    decode_open(&encoded).map_err(|_| RelayError::InvalidControl)
}

async fn open_session(
    quotas: &Mutex<AdmissionControl>,
    principal: PrincipalId,
    observer: &dyn RelayObserver,
) -> Result<SessionLease, RelayError> {
    quotas
        .lock()
        .await
        .open_session(principal)
        .map_err(|error| match error {
            AdmissionError::SessionQuotaExceeded => {
                observer.record(QuicRelayEvent::SessionQuotaRejected);
                RelayError::SessionQuotaRejected
            }
            _ => RelayError::Transport,
        })
}

async fn open_flow(
    quotas: &Mutex<AdmissionControl>,
    principal: PrincipalId,
    lease: SessionLease,
    observer: &dyn RelayObserver,
) -> Result<(), RelayError> {
    quotas
        .lock()
        .await
        .open_flow(principal, lease)
        .map_err(|error| match error {
            AdmissionError::FlowQuotaExceeded => {
                observer.record(QuicRelayEvent::FlowQuotaRejected);
                RelayError::FlowQuotaRejected
            }
            _ => RelayError::Transport,
        })
}

async fn relay_parts(
    send: &mut quinn::SendStream,
    receive: &mut quinn::RecvStream,
    target: SocketAddr,
    connect_timeout: Duration,
) -> Result<(), RelayError> {
    let mut target = timeout(connect_timeout, TcpStream::connect(target))
        .await
        .map_err(|_| RelayError::ConnectFailed)?
        .map_err(|_| RelayError::ConnectFailed)?;
    target
        .set_nodelay(true)
        .map_err(|_| RelayError::Transport)?;
    let (mut target_read, mut target_write) = target.split();
    let client_to_target = async {
        copy_quic_to_tcp(receive, &mut target_write).await?;
        target_write
            .shutdown()
            .await
            .map_err(|_| RelayError::Transport)
    };
    let target_to_client = async {
        copy_tcp_to_quic(&mut target_read, send).await?;
        send.finish().map_err(|_| RelayError::Transport)
    };
    tokio::try_join!(client_to_target, target_to_client).map(|_| ())
}

async fn copy_quic_to_tcp(
    receive: &mut quinn::RecvStream,
    target_write: &mut tokio::net::tcp::WriteHalf<'_>,
) -> Result<(), RelayError> {
    let mut buffer = vec![0; RELAY_COPY_BUFFER_BYTES];
    while let Some(read) = receive
        .read(&mut buffer)
        .await
        .map_err(|_| RelayError::Transport)?
    {
        target_write
            .write_all(&buffer[..read])
            .await
            .map_err(|_| RelayError::Transport)?;
    }
    Ok(())
}

async fn copy_tcp_to_quic(
    target_read: &mut tokio::net::tcp::ReadHalf<'_>,
    send: &mut quinn::SendStream,
) -> Result<(), RelayError> {
    let mut buffer = vec![0; RELAY_COPY_BUFFER_BYTES];
    loop {
        let read = target_read
            .read(&mut buffer)
            .await
            .map_err(|_| RelayError::Transport)?;
        if read == 0 {
            return Ok(());
        }
        send.write_all(&buffer[..read])
            .await
            .map_err(|_| RelayError::Transport)?;
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OpenRequest {
    pub secret: Vec<u8>,
    pub target: SocketAddr,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ControlError {
    Invalid,
    SecretTooLarge,
}

/// Encodes one request as `secret-length | secret | UTF-8 socket address`.
pub fn encode_open(request: &OpenRequest) -> Result<Vec<u8>, ControlError> {
    if request.secret.is_empty() {
        return Err(ControlError::Invalid);
    }
    let secret_length =
        u16::try_from(request.secret.len()).map_err(|_| ControlError::SecretTooLarge)?;
    if request.secret.len() > MAX_SECRET_BYTES {
        return Err(ControlError::SecretTooLarge);
    }
    let target = request.target.to_string();
    let target_length = u16::try_from(target.len()).map_err(|_| ControlError::Invalid)?;
    let mut encoded = Vec::with_capacity(HEADER_BYTES + request.secret.len() + target.len());
    encoded.extend_from_slice(&secret_length.to_be_bytes());
    encoded.extend_from_slice(&target_length.to_be_bytes());
    encoded.extend_from_slice(&request.secret);
    encoded.extend_from_slice(target.as_bytes());
    Ok(encoded)
}

pub fn decode_open(bytes: &[u8]) -> Result<OpenRequest, ControlError> {
    if bytes.len() < HEADER_BYTES {
        return Err(ControlError::Invalid);
    }
    let secret_length = usize::from(u16::from_be_bytes([bytes[0], bytes[1]]));
    let target_length = usize::from(u16::from_be_bytes([bytes[2], bytes[3]]));
    if secret_length == 0
        || secret_length > MAX_SECRET_BYTES
        || bytes.len() != HEADER_BYTES + secret_length + target_length
    {
        return Err(ControlError::Invalid);
    }
    let secret_end = HEADER_BYTES + secret_length;
    let target = std::str::from_utf8(&bytes[secret_end..])
        .ok()
        .and_then(|value| value.parse().ok())
        .ok_or(ControlError::Invalid)?;
    Ok(OpenRequest {
        secret: bytes[HEADER_BYTES..secret_end].to_vec(),
        target,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_record_round_trips_exact_ip_target() {
        let request = OpenRequest {
            secret: vec![7; 32],
            target: "192.0.2.10:443".parse().expect("target"),
        };
        assert_eq!(
            decode_open(&encode_open(&request).expect("encode")),
            Ok(request)
        );
    }

    #[test]
    fn malformed_and_oversize_records_are_rejected() {
        assert_eq!(decode_open(&[]), Err(ControlError::Invalid));
        assert_eq!(
            encode_open(&OpenRequest {
                secret: vec![0; 129],
                target: "192.0.2.10:443".parse().expect("target"),
            }),
            Err(ControlError::SecretTooLarge)
        );
    }

    #[test]
    fn relay_configuration_is_versioned_and_bounded() {
        assert!(QuicRelayConfig::default().validate().is_ok());
        assert_eq!(
            QuicRelayConfig {
                schema_version: 2,
                ..QuicRelayConfig::default()
            }
            .validate(),
            Err(ConfigError::UnsupportedSchema)
        );
        assert_eq!(
            QuicRelayConfig {
                max_control_bytes: 1,
                ..QuicRelayConfig::default()
            }
            .validate(),
            Err(ConfigError::ControlLimitTooSmall)
        );
        assert_eq!(
            QuicRelayConfig {
                control_timeout: Duration::ZERO,
                ..QuicRelayConfig::default()
            }
            .validate(),
            Err(ConfigError::ZeroControlTimeout)
        );
        assert_eq!(
            QuicRelayConfig {
                shutdown_timeout: Duration::ZERO,
                ..QuicRelayConfig::default()
            }
            .validate(),
            Err(ConfigError::ZeroShutdownTimeout)
        );
        assert_eq!(
            QuicRelayConfig {
                max_connections: 0,
                ..QuicRelayConfig::default()
            }
            .validate(),
            Err(ConfigError::ZeroConnectionLimit)
        );
        assert_eq!(
            QuicRelayConfig {
                max_streams_per_connection: 0,
                ..QuicRelayConfig::default()
            }
            .validate(),
            Err(ConfigError::ZeroStreamLimit)
        );
    }

    #[tokio::test]
    async fn streams_share_one_connection_session_lease() {
        let principal = PrincipalId(7);
        let admission = RelayAdmission {
            authenticator: Arc::new(
                Authenticator::new([velum_server::PrincipalCredential::new(
                    principal,
                    vec![7; 32],
                )
                .expect("credential")])
                .expect("authenticator"),
            ),
            destinations: Arc::new(DestinationPolicy::default()),
            quotas: Arc::new(Mutex::new(AdmissionControl::new(
                velum_server::PrincipalQuota {
                    max_sessions: 1,
                    max_flows_per_session: 2,
                },
            ))),
        };
        let connection = ConnectionAdmission::default();
        let observer = NoopRelayObserver;

        let first = connection
            .reserve_flow(&admission, principal, &observer)
            .await
            .expect("first flow");
        let second = connection
            .reserve_flow(&admission, principal, &observer)
            .await
            .expect("second flow");
        assert_eq!(first, second);
        assert_eq!(
            connection
                .reserve_flow(&admission, principal, &observer)
                .await,
            Err(RelayError::FlowQuotaRejected)
        );

        connection.release_flow(&admission, first).await;
        connection.release_flow(&admission, second).await;
        connection.close(&admission).await;
        assert!(
            admission
                .quotas
                .lock()
                .await
                .open_session(principal)
                .is_ok()
        );
    }

    #[tokio::test]
    async fn datagram_admission_binds_the_connection_to_one_principal() {
        let principal = PrincipalId(7);
        let admission = RelayAdmission {
            authenticator: Arc::new(
                Authenticator::new([velum_server::PrincipalCredential::new(
                    principal,
                    vec![7; 32],
                )
                .expect("credential")])
                .expect("authenticator"),
            ),
            destinations: Arc::new(DestinationPolicy::default()),
            quotas: Arc::new(Mutex::new(AdmissionControl::new(
                velum_server::PrincipalQuota {
                    max_sessions: 1,
                    max_flows_per_session: 1,
                },
            ))),
        };
        let connection = ConnectionAdmission::default();
        let observer = NoopRelayObserver;

        connection
            .admit_datagram(&admission, &[7; 32], &observer)
            .await
            .expect("datagram credential");
        assert!(connection.is_authenticated().await);
        connection
            .reserve_flow(&admission, principal, &observer)
            .await
            .expect("stream shares datagram session");
        assert_eq!(
            connection
                .admit_datagram(&admission, &[8; 32], &observer)
                .await,
            Err(RelayError::AuthenticationRejected)
        );
        connection.close(&admission).await;
    }

    #[test]
    fn transport_profile_matches_the_listener_stream_limit() {
        let relay = QuicRelayConfig {
            max_streams_per_connection: 7,
            ..QuicRelayConfig::default()
        };
        assert_eq!(
            relay
                .transport_profile()
                .expect("transport profile")
                .max_bidirectional_streams,
            7
        );
    }
}

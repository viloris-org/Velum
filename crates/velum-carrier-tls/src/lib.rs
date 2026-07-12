//! TLS 1.3 over TCP carrier implementation behind the carrier contract.
//!
//! One authenticated TLS connection maps to one reliable byte stream. TLS has
//! no datagram semantics, session authentication, or session delivery state.

use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};

use rustls::{ClientConfig, ProtocolVersion, ServerConfig, pki_types::ServerName};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};
use tokio_rustls::{TlsAcceptor, TlsConnector, TlsStream as RustlsStream};
use velum_carrier_api::{
    Carrier, CarrierCapabilities, CarrierError, CarrierHealth, CarrierId, CarrierKind, Reliability,
    StreamRequest,
};

/// A reliable byte stream carried by one TLS 1.3 over TCP connection.
pub struct TlsStream {
    stream: RustlsStream<TcpStream>,
    closed: Arc<AtomicBool>,
}

impl TlsStream {
    pub async fn write_all(&mut self, bytes: &[u8]) -> Result<(), CarrierError> {
        if self.closed.load(Ordering::Acquire) {
            return Err(CarrierError::Closed);
        }
        self.stream
            .write_all(bytes)
            .await
            .map_err(|_| CarrierError::Transport)
    }

    pub async fn read_chunk(&mut self, maximum: usize) -> Result<Option<Vec<u8>>, CarrierError> {
        if self.closed.load(Ordering::Acquire) {
            return Err(CarrierError::Closed);
        }
        if maximum == 0 {
            return Ok(Some(Vec::new()));
        }
        let mut bytes = vec![0; maximum];
        let read = self
            .stream
            .read(&mut bytes)
            .await
            .map_err(|_| CarrierError::Transport)?;
        if read == 0 {
            return Ok(None);
        }
        bytes.truncate(read);
        Ok(Some(bytes))
    }

    pub async fn finish(&mut self) -> Result<(), CarrierError> {
        if self.closed.load(Ordering::Acquire) {
            return Err(CarrierError::Closed);
        }
        self.stream
            .shutdown()
            .await
            .map_err(|_| CarrierError::Transport)
    }
}

/// A TLS 1.3 connection which is consumed when its single reliable stream is
/// opened or accepted. Logical flow and acknowledgement state remain session-owned.
#[derive(Clone)]
pub struct TlsCarrier {
    id: CarrierId,
    stream: Arc<Mutex<Option<TlsStream>>>,
    healthy: Arc<Mutex<bool>>,
    closed: Arc<AtomicBool>,
}

impl TlsCarrier {
    pub async fn connect(
        id: CarrierId,
        tcp: TcpStream,
        config: Arc<ClientConfig>,
        server_name: ServerName<'static>,
    ) -> Result<Self, CarrierError> {
        let stream = TlsConnector::from(config)
            .connect(server_name, tcp)
            .await
            .map_err(|_| CarrierError::Transport)?;
        if stream.get_ref().1.protocol_version() != Some(ProtocolVersion::TLSv1_3) {
            return Err(CarrierError::Transport);
        }
        Ok(Self::new(id, RustlsStream::Client(stream)))
    }

    pub async fn accept(
        id: CarrierId,
        tcp: TcpStream,
        config: Arc<ServerConfig>,
    ) -> Result<Self, CarrierError> {
        let stream = TlsAcceptor::from(config)
            .accept(tcp)
            .await
            .map_err(|_| CarrierError::Transport)?;
        if stream.get_ref().1.protocol_version() != Some(ProtocolVersion::TLSv1_3) {
            return Err(CarrierError::Transport);
        }
        Ok(Self::new(id, RustlsStream::Server(stream)))
    }

    fn new(id: CarrierId, stream: RustlsStream<TcpStream>) -> Self {
        let closed = Arc::new(AtomicBool::new(false));
        Self {
            id,
            stream: Arc::new(Mutex::new(Some(TlsStream {
                stream,
                closed: Arc::clone(&closed),
            }))),
            healthy: Arc::new(Mutex::new(true)),
            closed,
        }
    }

    fn take_stream(&self) -> Result<TlsStream, CarrierError> {
        let mut stream = self.stream.lock().map_err(|_| CarrierError::Transport)?;
        let taken = stream.take().ok_or(CarrierError::Closed)?;
        Ok(taken)
    }
}

impl Carrier for TlsCarrier {
    type ReliableStream = TlsStream;

    fn id(&self) -> CarrierId {
        self.id
    }

    fn kind(&self) -> CarrierKind {
        CarrierKind::Tls
    }

    fn capabilities(&self) -> CarrierCapabilities {
        CarrierCapabilities {
            streams: true,
            datagrams: false,
            max_datagram_payload: None,
        }
    }

    fn health(&self) -> CarrierHealth {
        let is_healthy = self.healthy.lock().map(|healthy| *healthy).unwrap_or(false);
        CarrierHealth {
            round_trip_time_millis: None,
            loss_parts_per_million: None,
            is_healthy,
        }
    }

    async fn open_reliable_stream(&self, _: StreamRequest) -> Result<TlsStream, CarrierError> {
        self.take_stream()
    }

    async fn accept_reliable_stream(&self) -> Result<TlsStream, CarrierError> {
        self.take_stream()
    }

    async fn send_datagram(&self, _: &[u8]) -> Result<(), CarrierError> {
        Err(CarrierError::UnsupportedReliability(Reliability::Datagram))
    }

    async fn receive_datagram(&self) -> Result<Vec<u8>, CarrierError> {
        Err(CarrierError::UnsupportedReliability(Reliability::Datagram))
    }

    fn close(&self) {
        self.closed.store(true, Ordering::Release);
        if let Ok(mut stream) = self.stream.lock() {
            stream.take();
        }
        if let Ok(mut healthy) = self.healthy.lock() {
            *healthy = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use rcgen::CertifiedKey;
    use rustls::{
        ClientConfig, RootCertStore, ServerConfig,
        pki_types::{CertificateDer, PrivateKeyDer, ServerName},
    };
    use tokio::net::TcpListener;
    use velum_carrier_api::{Carrier, CarrierId, CarrierKind, Reliability, StreamRequest};
    use velum_protocol::{Epoch, FlowId};

    use super::*;

    fn server_config(
        certificate: CertificateDer<'static>,
        key: PrivateKeyDer<'static>,
    ) -> Arc<ServerConfig> {
        Arc::new(
            ServerConfig::builder()
                .with_no_client_auth()
                .with_single_cert(vec![certificate], key)
                .expect("server config"),
        )
    }

    fn client_config(certificate: CertificateDer<'static>) -> Arc<ClientConfig> {
        let mut roots = RootCertStore::empty();
        roots.add(certificate).expect("root certificate");
        Arc::new(
            ClientConfig::builder()
                .with_root_certificates(roots)
                .with_no_client_auth(),
        )
    }

    #[tokio::test]
    async fn tls_13_carriers_exchange_bytes_and_reject_datagrams() {
        let CertifiedKey { cert, signing_key } =
            rcgen::generate_simple_self_signed(vec!["localhost".into()]).expect("certificate");
        let certificate = cert.der().clone();
        let server = server_config(certificate.clone(), signing_key.into());
        let client = client_config(certificate);
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("listener");
        let address = listener.local_addr().expect("address");

        let server_task = tokio::spawn(async move {
            let (tcp, _) = listener.accept().await.expect("accept");
            let carrier = TlsCarrier::accept(CarrierId(2), tcp, server)
                .await
                .expect("TLS 1.3 accept");
            assert_eq!(carrier.kind(), CarrierKind::Tls);
            assert!(!carrier.capabilities().supports(Reliability::Datagram));
            let mut stream = carrier.accept_reliable_stream().await.expect("stream");
            assert_eq!(stream.read_chunk(32).await, Ok(Some(b"ping".to_vec())));
            stream.write_all(b"pong").await.expect("write");
            stream.finish().await.expect("finish");
        });

        let tcp = TcpStream::connect(address).await.expect("connect");
        let carrier = TlsCarrier::connect(
            CarrierId(1),
            tcp,
            client,
            ServerName::try_from("localhost").expect("name").to_owned(),
        )
        .await
        .expect("TLS 1.3 connect");
        assert!(carrier.health().is_healthy);
        assert_eq!(
            carrier.send_datagram(b"not a datagram").await,
            Err(CarrierError::UnsupportedReliability(Reliability::Datagram))
        );
        let mut stream = carrier
            .open_reliable_stream(StreamRequest {
                flow_id: FlowId(0),
                epoch: Epoch(0),
            })
            .await
            .expect("stream");
        stream.write_all(b"ping").await.expect("write");
        assert_eq!(stream.read_chunk(32).await, Ok(Some(b"pong".to_vec())));
        assert!(matches!(
            carrier
                .open_reliable_stream(StreamRequest {
                    flow_id: FlowId(1),
                    epoch: Epoch(0)
                })
                .await,
            Err(CarrierError::Closed)
        ));
        carrier.close();
        assert_eq!(
            stream.write_all(b"after close").await,
            Err(CarrierError::Closed)
        );
        server_task.await.expect("server task");
    }

    #[test]
    fn tls_carrier_implements_the_public_contract() {
        fn requires_carrier_contract<T: Carrier<ReliableStream = TlsStream>>() {}
        requires_carrier_contract::<TlsCarrier>();
    }
}

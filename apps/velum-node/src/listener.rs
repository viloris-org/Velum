//! QUIC listener lifecycle for the Stage 2 relay.
//!
//! TLS certificates are deliberately supplied as Quinn configuration by the
//! deployment boundary. This module owns only accepting connections, dispatching
//! their bidirectional streams, and bounded shutdown.

use std::{future::Future, net::SocketAddr, sync::Arc};

use tokio::{sync::Semaphore, task::JoinSet, time::timeout};
use velum_carrier_quic::{CarrierId, QuicCarrier};
use velum_telemetry::QuicRelayEvent;

use crate::{
    ConnectionAdmission, QuicRelayConfig, RelayAdmission, RelayObserver, relay_quic_stream,
};

/// Binds a server endpoint using deployment-provided TLS and transport settings.
pub fn bind_quic_listener(
    address: SocketAddr,
    server_config: quinn::ServerConfig,
) -> Result<quinn::Endpoint, std::io::Error> {
    quinn::Endpoint::server(server_config, address)
}

/// Serves incoming QUIC connections until `shutdown` resolves.
///
/// Every accepted bidirectional stream runs independently, so a blocked target
/// does not prevent the connection from presenting another flow. Endpoint close
/// signals all active connections, then unfinished tasks are aborted after the
/// configured drain budget.
pub async fn serve_quic_listener(
    endpoint: quinn::Endpoint,
    admission: RelayAdmission,
    config: QuicRelayConfig,
    observer: Arc<dyn RelayObserver>,
    shutdown: impl Future<Output = ()> + Send,
) {
    if config.validate().is_err() {
        return;
    }

    tokio::pin!(shutdown);
    let connection_slots = Arc::new(Semaphore::new(config.max_connections));
    let mut connections = JoinSet::new();
    loop {
        tokio::select! {
            _ = &mut shutdown => break,
            incoming = endpoint.accept() => match incoming {
                Some(incoming) => {
                    let Ok(slot) = Arc::clone(&connection_slots).try_acquire_owned() else {
                        observer.record(QuicRelayEvent::ConnectionRejected);
                        incoming.refuse();
                        continue;
                    };
                    let admission = admission.clone();
                    let config = config.clone();
                    let observer = Arc::clone(&observer);
                    connections.spawn(async move {
                        let _slot = slot;
                        let Ok(connection) = incoming.await else {
                            return;
                        };
                        observer.record(QuicRelayEvent::ConnectionAccepted);
                        serve_connection(connection, admission, config, observer).await;
                    });
                }
                None => break,
            },
            Some(_) = connections.join_next(), if !connections.is_empty() => {}
        }
    }

    observer.record(QuicRelayEvent::ShutdownStarted);
    endpoint.close(0_u32.into(), b"velum shutdown");
    if timeout(config.shutdown_timeout, async {
        while connections.join_next().await.is_some() {}
    })
    .await
    .is_err()
    {
        connections.abort_all();
        while connections.join_next().await.is_some() {}
    }
}

async fn serve_connection(
    connection: quinn::Connection,
    admission: RelayAdmission,
    config: QuicRelayConfig,
    observer: Arc<dyn RelayObserver>,
) {
    let carrier = QuicCarrier::new(CarrierId(0), connection);
    let connection_admission = ConnectionAdmission::default();
    let stream_slots = Arc::new(Semaphore::new(config.max_streams_per_connection));
    let mut flows = JoinSet::new();
    loop {
        tokio::select! {
            stream = carrier.accept_stream() => match stream {
                Ok(stream) => {
                    let Ok(slot) = Arc::clone(&stream_slots).try_acquire_owned() else {
                        observer.record(QuicRelayEvent::StreamRejected);
                        continue;
                    };
                    let admission = admission.clone();
                    let connection_admission = connection_admission.clone();
                    let config = config.clone();
                    let observer = Arc::clone(&observer);
                    flows.spawn(async move {
                        let _slot = slot;
                        let _ = relay_quic_stream(
                            stream,
                            &admission,
                            &connection_admission,
                            &config,
                            observer.as_ref(),
                        )
                        .await;
                    });
                }
                Err(_) => break,
            },
            Some(_) = flows.join_next(), if !flows.is_empty() => {}
        }
    }

    while flows.join_next().await.is_some() {}
    connection_admission.close(&admission).await;
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use tokio::sync::oneshot;
    use velum_server::{
        AdmissionControl, Authenticator, DestinationPolicy, PrincipalCredential, PrincipalId,
        PrincipalQuota,
    };

    use super::*;

    struct AcceptedObserver(Mutex<Option<oneshot::Sender<()>>>);

    impl RelayObserver for AcceptedObserver {
        fn record(&self, event: QuicRelayEvent) {
            if event == QuicRelayEvent::ConnectionAccepted
                && let Some(sender) = self.0.lock().expect("observer lock").take()
            {
                let _ = sender.send(());
            }
        }
    }

    fn test_admission() -> RelayAdmission {
        RelayAdmission {
            authenticator: Arc::new(
                Authenticator::new([
                    PrincipalCredential::new(PrincipalId(1), vec![7; 32]).expect("credential")
                ])
                .expect("authenticator"),
            ),
            destinations: Arc::new(DestinationPolicy::default()),
            quotas: Arc::new(tokio::sync::Mutex::new(AdmissionControl::new(
                PrincipalQuota {
                    max_sessions: 1,
                    max_flows_per_session: 1,
                },
            ))),
        }
    }

    #[tokio::test]
    async fn listener_accepts_a_real_quic_connection_and_stops() {
        let certified =
            rcgen::generate_simple_self_signed(vec!["localhost".to_owned()]).expect("certificate");
        let certificate = rustls::pki_types::CertificateDer::from(certified.cert);
        let key =
            rustls::pki_types::PrivatePkcs8KeyDer::from(certified.signing_key.serialize_der());
        let server_config =
            quinn::ServerConfig::with_single_cert(vec![certificate.clone()], key.into())
                .expect("server config");
        let endpoint =
            match bind_quic_listener("127.0.0.1:0".parse().expect("address"), server_config) {
                Ok(endpoint) => endpoint,
                Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => return,
                Err(error) => panic!("listener: {error}"),
            };
        let address = endpoint.local_addr().expect("listener address");

        let (accepted_sender, accepted) = oneshot::channel();
        let observer = Arc::new(AcceptedObserver(Mutex::new(Some(accepted_sender))));
        let (shutdown_sender, shutdown) = oneshot::channel();
        let listener = tokio::spawn(serve_quic_listener(
            endpoint,
            test_admission(),
            QuicRelayConfig::default(),
            observer,
            async move {
                let _ = shutdown.await;
            },
        ));

        let mut roots = rustls::RootCertStore::empty();
        roots.add(certificate).expect("trusted certificate");
        let client_config =
            quinn::ClientConfig::with_root_certificates(Arc::new(roots)).expect("client config");
        let mut client = quinn::Endpoint::client("127.0.0.1:0".parse().expect("address"))
            .expect("client endpoint");
        client.set_default_client_config(client_config);
        let connection = client
            .connect(address, "localhost")
            .expect("connect request")
            .await
            .expect("connection");
        accepted.await.expect("accepted event");

        shutdown_sender.send(()).expect("shutdown receiver");
        listener.await.expect("listener task");
        connection.close(0_u32.into(), b"test complete");
        client.close(0_u32.into(), b"test complete");
    }
}

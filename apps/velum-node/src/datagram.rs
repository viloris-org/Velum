//! Bounded UDP associations over QUIC DATAGRAM after listener admission.

use std::{
    collections::HashMap,
    net::{IpAddr, SocketAddr},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use tokio::{
    net::UdpSocket,
    sync::{Mutex as AsyncMutex, watch},
};
use velum_protocol::{Datagram, DatagramSessionId};
use velum_server::DestinationPolicy;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DatagramRelayConfig {
    pub max_associations: usize,
    pub idle_timeout: Duration,
    pub receive_buffer_bytes: usize,
}

impl Default for DatagramRelayConfig {
    fn default() -> Self {
        Self {
            max_associations: 256,
            idle_timeout: Duration::from_secs(60),
            receive_buffer_bytes: 64 * 1024,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DatagramRelayConfigError {
    AssociationLimit,
    IdleTimeout,
    ReceiveBuffer,
}

impl DatagramRelayConfig {
    pub fn validate(&self) -> Result<(), DatagramRelayConfigError> {
        if self.max_associations == 0 {
            return Err(DatagramRelayConfigError::AssociationLimit);
        }
        if self.idle_timeout.is_zero() {
            return Err(DatagramRelayConfigError::IdleTimeout);
        }
        if self.receive_buffer_bytes == 0 {
            return Err(DatagramRelayConfigError::ReceiveBuffer);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DatagramRelayError {
    Malformed,
    UnexpectedDirection,
    DestinationRejected,
    SessionTargetMismatch,
    AssociationLimit,
    Socket,
}

trait DatagramSink: Send + Sync {
    fn send(&self, bytes: Vec<u8>) -> Result<(), ()>;
}

struct QuinnDatagramSink(quinn::Connection);

impl DatagramSink for QuinnDatagramSink {
    fn send(&self, bytes: Vec<u8>) -> Result<(), ()> {
        self.0.send_datagram(bytes.into()).map_err(|_| ())
    }
}

#[derive(Clone)]
pub struct DatagramRelay {
    config: DatagramRelayConfig,
    sink: Arc<dyn DatagramSink>,
    associations: Arc<AsyncMutex<HashMap<DatagramSessionId, Association>>>,
}

#[derive(Clone)]
struct Association {
    destination: SocketAddr,
    socket: Arc<UdpSocket>,
    last_active: Arc<Mutex<Instant>>,
    cancel: watch::Sender<()>,
}

impl DatagramRelay {
    pub fn for_quic(
        connection: quinn::Connection,
        config: DatagramRelayConfig,
    ) -> Result<Self, DatagramRelayConfigError> {
        Self::new(Arc::new(QuinnDatagramSink(connection)), config)
    }

    fn new(
        sink: Arc<dyn DatagramSink>,
        config: DatagramRelayConfig,
    ) -> Result<Self, DatagramRelayConfigError> {
        config.validate()?;
        Ok(Self {
            config,
            sink,
            associations: Arc::new(AsyncMutex::new(HashMap::new())),
        })
    }

    pub async fn forward(
        &self,
        encoded: &[u8],
        destinations: &DestinationPolicy,
    ) -> Result<(), DatagramRelayError> {
        self.prune_idle().await;
        let (session_id, destination, payload) = match Datagram::decode(encoded) {
            Ok(Datagram::ClientToServer {
                credential: _,
                session_id,
                destination,
                payload,
            }) => (session_id, destination, payload),
            Ok(Datagram::ServerToClient { .. }) => {
                return Err(DatagramRelayError::UnexpectedDirection);
            }
            Err(_) => return Err(DatagramRelayError::Malformed),
        };
        if !destinations.allows(destination) {
            return Err(DatagramRelayError::DestinationRejected);
        }
        let association = self.association(session_id, destination).await?;
        touch(&association.last_active);
        association
            .socket
            .send(&payload)
            .await
            .map_err(|_| DatagramRelayError::Socket)?;
        Ok(())
    }

    pub async fn shutdown(&self) {
        let mut associations = self.associations.lock().await;
        for association in associations.values() {
            let _ = association.cancel.send(());
        }
        associations.clear();
    }

    async fn association(
        &self,
        session_id: DatagramSessionId,
        destination: SocketAddr,
    ) -> Result<Association, DatagramRelayError> {
        let mut associations = self.associations.lock().await;
        if let Some(existing) = associations.get(&session_id) {
            return if existing.destination == destination {
                Ok(existing.clone())
            } else {
                Err(DatagramRelayError::SessionTargetMismatch)
            };
        }
        if associations.len() >= self.config.max_associations {
            return Err(DatagramRelayError::AssociationLimit);
        }

        let bind_address = match destination.ip() {
            IpAddr::V4(_) => "0.0.0.0:0",
            IpAddr::V6(_) => "[::]:0",
        };
        let socket = Arc::new(
            UdpSocket::bind(bind_address)
                .await
                .map_err(|_| DatagramRelayError::Socket)?,
        );
        socket
            .connect(destination)
            .await
            .map_err(|_| DatagramRelayError::Socket)?;
        let (cancel, receiver) = watch::channel(());
        let association = Association {
            destination,
            socket: Arc::clone(&socket),
            last_active: Arc::new(Mutex::new(Instant::now())),
            cancel,
        };
        tokio::spawn(receive_responses(
            socket,
            session_id,
            destination,
            Arc::clone(&association.last_active),
            Arc::clone(&self.sink),
            self.config.receive_buffer_bytes,
            receiver,
        ));
        associations.insert(session_id, association.clone());
        Ok(association)
    }

    async fn prune_idle(&self) {
        let mut associations = self.associations.lock().await;
        associations.retain(|_, association| {
            let active = association
                .last_active
                .lock()
                .map(|last_active| last_active.elapsed() <= self.config.idle_timeout)
                .unwrap_or(false);
            if !active {
                let _ = association.cancel.send(());
            }
            active
        });
    }
}

async fn receive_responses(
    socket: Arc<UdpSocket>,
    session_id: DatagramSessionId,
    source: SocketAddr,
    last_active: Arc<Mutex<Instant>>,
    sink: Arc<dyn DatagramSink>,
    receive_buffer_bytes: usize,
    mut cancel: watch::Receiver<()>,
) {
    let mut buffer = vec![0; receive_buffer_bytes];
    loop {
        tokio::select! {
            changed = cancel.changed() => {
                if changed.is_ok() {
                    return;
                }
            }
            received = socket.recv(&mut buffer) => {
                let Ok(received) = received else {
                    return;
                };
                touch(&last_active);
                let response = Datagram::ServerToClient {
                    session_id,
                    source,
                    payload: buffer[..received].to_vec(),
                };
                let Ok(encoded) = response.encode() else {
                    return;
                };
                if sink.send(encoded).is_err() {
                    return;
                }
            }
        }
    }
}

fn touch(last_active: &Mutex<Instant>) {
    if let Ok(mut value) = last_active.lock() {
        *value = Instant::now();
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex as StdMutex;

    use tokio::{sync::Notify, time::timeout};
    use velum_protocol::DatagramSessionId;

    use super::*;

    #[derive(Default)]
    struct RecordingSink {
        packets: StdMutex<Vec<Vec<u8>>>,
        received: Notify,
    }

    impl DatagramSink for RecordingSink {
        fn send(&self, bytes: Vec<u8>) -> Result<(), ()> {
            self.packets.lock().expect("packets lock").push(bytes);
            self.received.notify_one();
            Ok(())
        }
    }

    #[tokio::test]
    async fn authorized_datagram_round_trips_through_a_bounded_udp_association() {
        let target = UdpSocket::bind("127.0.0.1:0").await.expect("target");
        let target_address = target.local_addr().expect("target address");
        tokio::spawn(async move {
            let mut buffer = [0; 64];
            let (read, peer) = target.recv_from(&mut buffer).await.expect("request");
            target
                .send_to(&buffer[..read], peer)
                .await
                .expect("response");
        });
        let sink = Arc::new(RecordingSink::default());
        let relay = DatagramRelay::new(sink.clone(), DatagramRelayConfig::default())
            .expect("relay configuration");
        let request = Datagram::ClientToServer {
            credential: vec![7; 32],
            session_id: DatagramSessionId::new(1).expect("session"),
            destination: target_address,
            payload: b"ping".to_vec(),
        }
        .encode()
        .expect("request");
        let received = sink.received.notified();
        relay
            .forward(&request, &DestinationPolicy::new([target_address]))
            .await
            .expect("forward");
        timeout(Duration::from_secs(1), received)
            .await
            .expect("response");
        let response = {
            let packets = sink.packets.lock().expect("packets lock");
            assert_eq!(packets.len(), 1);
            Datagram::decode(&packets[0])
        };
        assert_eq!(
            response,
            Ok(Datagram::ServerToClient {
                session_id: DatagramSessionId::new(1).expect("session"),
                source: target_address,
                payload: b"ping".to_vec(),
            })
        );
        relay.shutdown().await;
    }

    #[tokio::test]
    async fn relay_rejects_unapproved_and_repurposed_sessions() {
        let sink = Arc::new(RecordingSink::default());
        let relay =
            DatagramRelay::new(sink, DatagramRelayConfig::default()).expect("relay configuration");
        let session_id = DatagramSessionId::new(1).expect("session");
        let rejected = Datagram::ClientToServer {
            credential: vec![7; 32],
            session_id,
            destination: "192.0.2.1:53".parse().expect("address"),
            payload: vec![],
        }
        .encode()
        .expect("request");
        assert_eq!(
            relay
                .forward(&rejected, &DestinationPolicy::default())
                .await,
            Err(DatagramRelayError::DestinationRejected)
        );
    }
}

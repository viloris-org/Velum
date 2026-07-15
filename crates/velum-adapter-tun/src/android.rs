use std::{
    collections::BTreeMap,
    io,
    net::SocketAddr,
    os::fd::FromRawFd,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

use ipstack::{IpStack, IpStackConfig, IpStackStream};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    sync::{Mutex, Semaphore, mpsc, watch},
    task::JoinHandle,
};
use velum_client_runtime::{ClientRuntime, DatagramSessionId};

const MAX_TCP_FLOWS: usize = 256;
const MAX_UDP_FLOWS: usize = 256;
const BUFFER_BYTES: usize = 16 * 1024;

type UdpRoutes = Arc<Mutex<BTreeMap<DatagramSessionId, (SocketAddr, mpsc::Sender<Vec<u8>>)>>>;

/// Drives an Android-owned raw TUN descriptor until cancellation or failure.
pub async fn run_android_tun(
    runtime: Arc<ClientRuntime>,
    tun_fd: i32,
    mtu: u16,
    mut shutdown: watch::Receiver<bool>,
) -> io::Result<()> {
    if tun_fd < 0 || !runtime.is_generation_online(runtime.snapshot().generation) {
        return Err(io::Error::new(
            io::ErrorKind::NotConnected,
            "runtime or TUN descriptor is unavailable",
        ));
    }
    let file = unsafe { std::fs::File::from_raw_fd(tun_fd) };
    let device = tokio::fs::File::from_std(file);
    let mut config = IpStackConfig::default();
    config
        .mtu(mtu)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;
    config.udp_timeout(Duration::from_secs(30));
    let mut stack = IpStack::new(config, device);
    let routes: UdpRoutes = Arc::new(Mutex::new(BTreeMap::new()));
    let datagram_receiver =
        spawn_datagram_receiver(Arc::clone(&runtime), Arc::clone(&routes), shutdown.clone());
    let tcp_slots = Arc::new(Semaphore::new(MAX_TCP_FLOWS));
    let udp_slots = Arc::new(Semaphore::new(MAX_UDP_FLOWS));

    loop {
        tokio::select! {
            changed = shutdown.changed() => {
                if changed.is_err() || *shutdown.borrow() { break; }
            }
            accepted = stack.accept() => match accepted {
                Ok(IpStackStream::Tcp(stream)) => {
                    let Ok(slot) = Arc::clone(&tcp_slots).try_acquire_owned() else { continue; };
                    let runtime = Arc::clone(&runtime);
                    let stopped = shutdown.clone();
                    tokio::spawn(async move {
                        let _slot = slot;
                        let _ = relay_tcp(runtime, stream, stopped).await;
                    });
                }
                Ok(IpStackStream::Udp(stream)) => {
                    let Ok(slot) = Arc::clone(&udp_slots).try_acquire_owned() else { continue; };
                    let runtime = Arc::clone(&runtime);
                    let routes = Arc::clone(&routes);
                    let stopped = shutdown.clone();
                    tokio::spawn(async move {
                        let _slot = slot;
                        let _ = relay_udp(runtime, routes, stream, stopped).await;
                    });
                }
                Ok(IpStackStream::UnknownTransport(_) | IpStackStream::UnknownNetwork(_)) => {}
                Err(error) => {
                    datagram_receiver.abort();
                    return Err(io::Error::other(error));
                }
            }
        }
    }
    datagram_receiver.abort();
    routes.lock().await.clear();
    Ok(())
}

async fn relay_tcp(
    runtime: Arc<ClientRuntime>,
    tun: ipstack::IpStackTcpStream,
    mut shutdown: watch::Receiver<bool>,
) -> io::Result<()> {
    let target = tun.peer_addr();
    let stream = runtime.open_stream(target).await.map_err(runtime_error)?;
    let (_, mut send, mut receive) = stream.into_parts();
    let (mut tun_reader, mut tun_writer) = tokio::io::split(tun);
    let upstream = async {
        let mut buffer = [0_u8; BUFFER_BYTES];
        loop {
            let count = tun_reader.read(&mut buffer).await?;
            if count == 0 {
                return send.finish().map_err(runtime_error);
            }
            send.write_all(&buffer[..count])
                .await
                .map_err(runtime_error)?;
        }
    };
    let downstream = async {
        let mut buffer = [0_u8; BUFFER_BYTES];
        loop {
            let Some(count) = receive.read(&mut buffer).await.map_err(runtime_error)? else {
                return tun_writer.shutdown().await;
            };
            tun_writer.write_all(&buffer[..count]).await?;
        }
    };
    tokio::select! {
        result = upstream => result,
        result = downstream => result,
        _ = shutdown.changed() => Ok(()),
    }
}

async fn relay_udp(
    runtime: Arc<ClientRuntime>,
    routes: UdpRoutes,
    mut tun: ipstack::IpStackUdpStream,
    mut shutdown: watch::Receiver<bool>,
) -> io::Result<()> {
    static NEXT_SESSION: AtomicU64 = AtomicU64::new(1);
    let target = tun.peer_addr();
    let session = allocate_session(&routes, &NEXT_SESSION).await;
    let (responses, mut response_receiver) = mpsc::channel::<Vec<u8>>(32);
    routes.lock().await.insert(session, (target, responses));
    let mut buffer = [0_u8; 65_507];
    let result = loop {
        tokio::select! {
            read = tun.read(&mut buffer) => {
                let count = match read {
                    Ok(count) => count,
                    Err(error) => break Err(error),
                };
                if count == 0 { break Ok(()); }
                if let Err(error) = runtime
                    .send_datagram(session, target, &buffer[..count])
                    .await
                {
                    break Err(runtime_error(error));
                }
            }
            response = response_receiver.recv() => match response {
                Some(payload) => {
                    if let Err(error) = tun.write_all(&payload).await {
                        break Err(error);
                    }
                }
                None => break Ok(()),
            },
            _ = shutdown.changed() => break Ok(()),
        }
    };
    routes.lock().await.remove(&session);
    result
}

fn spawn_datagram_receiver(
    runtime: Arc<ClientRuntime>,
    routes: UdpRoutes,
    mut shutdown: watch::Receiver<bool>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            tokio::select! {
                response = runtime.receive_datagram() => {
                    let Ok(response) = response else { break; };
                    let route = routes.lock().await.get(&response.session_id).cloned();
                    if let Some((target, sender)) = route
                        && target == response.source
                    {
                        let _ = sender.send(response.payload).await;
                    }
                }
                _ = shutdown.changed() => break,
            }
        }
    })
}

async fn allocate_session(routes: &UdpRoutes, next: &AtomicU64) -> DatagramSessionId {
    loop {
        let value = next.fetch_add(1, Ordering::Relaxed).max(1);
        let session = DatagramSessionId::new(value).expect("session id is non-zero");
        if !routes.lock().await.contains_key(&session) {
            return session;
        }
    }
}

fn runtime_error(_: impl std::fmt::Debug) -> io::Error {
    io::Error::other("Velum runtime traffic operation failed")
}

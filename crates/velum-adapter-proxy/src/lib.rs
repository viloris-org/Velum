//! Loopback-only HTTP CONNECT and SOCKS5 adapter for an online Velum runtime.
//!
//! This adapter intentionally accepts only literal IP destinations. The Stage 2
//! relay contract authorizes `SocketAddr` values, so resolving domains locally
//! would create an unprotected DNS side channel.

use std::{
    io,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    sync::Arc,
};

use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::watch,
    task::JoinHandle,
};
use velum_client_runtime::{ClientRuntime, RuntimeReceiveStream, RuntimeSendStream};

const MAX_HANDSHAKE_BYTES: usize = 16 * 1024;
const COPY_BUFFER_BYTES: usize = 16 * 1024;

/// A running loopback proxy. Dropping it shuts down its listener and clients.
pub struct ProxyAdapter {
    address: SocketAddr,
    shutdown: watch::Sender<bool>,
    task: JoinHandle<()>,
}

impl ProxyAdapter {
    /// Binds an IPv4 loopback listener. `port == 0` selects an ephemeral port.
    pub async fn start(runtime: Arc<ClientRuntime>, port: u16) -> io::Result<Self> {
        if !runtime.is_generation_online(runtime.snapshot().generation) {
            return Err(io::Error::new(
                io::ErrorKind::NotConnected,
                "runtime is not online",
            ));
        }
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, port)).await?;
        let address = listener.local_addr()?;
        let (shutdown, mut stopped) = watch::channel(false);
        let task = tokio::spawn(async move {
            loop {
                tokio::select! {
                    changed = stopped.changed() => {
                        if changed.is_err() || *stopped.borrow() { break; }
                    }
                    accepted = listener.accept() => match accepted {
                        Ok((stream, _)) => {
                            let runtime = Arc::clone(&runtime);
                            tokio::spawn(async move { let _ = serve(stream, runtime).await; });
                        }
                        Err(_) => break,
                    },
                }
            }
        });
        Ok(Self {
            address,
            shutdown,
            task,
        })
    }

    pub const fn address(&self) -> SocketAddr {
        self.address
    }

    /// Stops accepting new clients and aborts the listener task.
    pub fn stop(self) {
        self.shutdown.send_replace(true);
        self.task.abort();
    }
}

impl Drop for ProxyAdapter {
    fn drop(&mut self) {
        self.shutdown.send_replace(true);
        self.task.abort();
    }
}

async fn serve(mut local: TcpStream, runtime: Arc<ClientRuntime>) -> io::Result<()> {
    let (target, protocol) = read_target(&mut local).await?;
    let stream = runtime.open_stream(target).await.map_err(runtime_error)?;
    let (generation, send, receive) = stream.into_parts();
    if !runtime.is_generation_online(generation) {
        return Err(io::Error::new(
            io::ErrorKind::NotConnected,
            "runtime generation ended",
        ));
    }
    write_success(&mut local, protocol).await?;
    relay(local, send, receive).await
}

async fn relay(
    local: TcpStream,
    mut send: RuntimeSendStream,
    mut receive: RuntimeReceiveStream,
) -> io::Result<()> {
    let (mut reader, mut writer) = local.into_split();
    let upstream = tokio::spawn(async move {
        let mut buffer = [0_u8; COPY_BUFFER_BYTES];
        loop {
            let count = reader.read(&mut buffer).await?;
            if count == 0 {
                return send.finish().map_err(runtime_error);
            }
            send.write_all(&buffer[..count])
                .await
                .map_err(runtime_error)?;
        }
    });
    let downstream = async {
        let mut buffer = [0_u8; COPY_BUFFER_BYTES];
        loop {
            let Some(count) = receive.read(&mut buffer).await.map_err(runtime_error)? else {
                return writer.shutdown().await;
            };
            writer.write_all(&buffer[..count]).await?;
        }
    }
    .await;
    upstream.abort();
    downstream
}

#[derive(Clone, Copy)]
enum ConnectProtocol {
    Http,
    Socks5,
}

async fn read_target(local: &mut TcpStream) -> io::Result<(SocketAddr, ConnectProtocol)> {
    let mut first = [0_u8; 1];
    local.read_exact(&mut first).await?;
    if first[0] == 5 {
        return read_socks5_target(local)
            .await
            .map(|target| (target, ConnectProtocol::Socks5));
    }
    read_http_connect_target(local, first[0])
        .await
        .map(|target| (target, ConnectProtocol::Http))
}

async fn read_socks5_target(local: &mut TcpStream) -> io::Result<SocketAddr> {
    let mut count = [0_u8; 1];
    local.read_exact(&mut count).await?;
    let mut methods = vec![0_u8; usize::from(count[0])];
    local.read_exact(&mut methods).await?;
    if !methods.contains(&0) {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "SOCKS authentication is required",
        ));
    }
    local.write_all(&[5, 0]).await?;
    let mut request = [0_u8; 4];
    local.read_exact(&mut request).await?;
    if request[..3] != [5, 1, 0] {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "only SOCKS CONNECT is supported",
        ));
    }
    let address = match request[3] {
        1 => {
            let mut value = [0_u8; 4];
            local.read_exact(&mut value).await?;
            IpAddr::V4(value.into())
        }
        4 => {
            let mut value = [0_u8; 16];
            local.read_exact(&mut value).await?;
            IpAddr::V6(value.into())
        }
        3 => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "domain destinations are not supported",
            ));
        }
        _ => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "invalid SOCKS address type",
            ));
        }
    };
    let mut port = [0_u8; 2];
    local.read_exact(&mut port).await?;
    let target = SocketAddr::new(address, u16::from_be_bytes(port));
    Ok(target)
}

async fn read_http_connect_target(local: &mut TcpStream, first: u8) -> io::Result<SocketAddr> {
    let mut request = vec![first];
    while !request.ends_with(b"\r\n\r\n") {
        if request.len() == MAX_HANDSHAKE_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "HTTP request header is too large",
            ));
        }
        let mut byte = [0_u8; 1];
        local.read_exact(&mut byte).await?;
        request.push(byte[0]);
    }
    let head = std::str::from_utf8(&request)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "HTTP request is not UTF-8"))?;
    let mut parts = head.split_whitespace();
    let method = parts
        .next()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "missing HTTP method"))?;
    let authority = parts
        .next()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "missing HTTP authority"))?;
    let target = authority.parse::<SocketAddr>().map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "HTTP proxy requires a literal IP host and port",
        )
    })?;
    if !method.eq_ignore_ascii_case("CONNECT") {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "only HTTP CONNECT is supported",
        ));
    }
    Ok(target)
}

async fn write_success(local: &mut TcpStream, protocol: ConnectProtocol) -> io::Result<()> {
    match protocol {
        ConnectProtocol::Http => {
            local
                .write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n")
                .await
        }
        ConnectProtocol::Socks5 => local.write_all(&[5, 0, 0, 1, 0, 0, 0, 0, 0, 0]).await,
    }
}

fn runtime_error(_: impl std::fmt::Debug) -> io::Error {
    io::Error::other("Velum runtime stream failed")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn literal_socket_addresses_cover_ipv4_and_ipv6() {
        assert_eq!(
            "192.0.2.1:443".parse::<SocketAddr>().expect("IPv4").port(),
            443
        );
        assert_eq!(
            "[2001:db8::1]:443"
                .parse::<SocketAddr>()
                .expect("IPv6")
                .port(),
            443
        );
    }
}

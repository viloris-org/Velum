//! Loopback-only HTTP and SOCKS5 adapter for an online Velum runtime.
//!
//! Domain targets are resolved before the proxy acknowledges a CONNECT request.
//! The resulting address remains subject to the relay's exact destination policy.

mod http;
mod target;

pub use velum_client_routing::{
    IpCidr, RouteContext, RoutingAction, RoutingError as RuleParseError, RoutingPolicy, RoutingRule,
};

use std::{
    io,
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr},
    sync::Arc,
};

use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::watch,
    task::JoinHandle,
};
use velum_client_runtime::{ClientRuntime, RuntimeReceiveStream, RuntimeSendStream};

use crate::{http::parse_forward_request, target::ProxyTarget};

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
        Self::start_with_policy(runtime, port, RoutingPolicy::default()).await
    }

    /// Binds a listener that applies `policy` to each incoming connection.
    pub async fn start_with_policy(
        runtime: Arc<ClientRuntime>,
        port: u16,
        policy: RoutingPolicy,
    ) -> io::Result<Self> {
        if !runtime.is_generation_online(runtime.snapshot().generation) {
            return Err(io::Error::new(
                io::ErrorKind::NotConnected,
                "runtime is not online",
            ));
        }
        let ipv4 = TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, port))).await?;
        let address = ipv4.local_addr()?;
        let ipv6 =
            TcpListener::bind(SocketAddr::from((Ipv6Addr::LOCALHOST, address.port()))).await?;
        Ok(Self::from_listeners(
            runtime,
            address,
            policy,
            vec![ipv4, ipv6],
        ))
    }

    /// Binds one explicit loopback address, allowing callers to compose IPv4 and IPv6 listeners.
    pub async fn start_on_loopback(
        runtime: Arc<ClientRuntime>,
        address: SocketAddr,
        policy: RoutingPolicy,
    ) -> io::Result<Self> {
        if !address.ip().is_loopback() {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "proxy listener must use a loopback address",
            ));
        }
        if !runtime.is_generation_online(runtime.snapshot().generation) {
            return Err(io::Error::new(
                io::ErrorKind::NotConnected,
                "runtime is not online",
            ));
        }
        let listener = TcpListener::bind(address).await?;
        let address = listener.local_addr()?;
        Ok(Self::from_listeners(
            runtime,
            address,
            policy,
            vec![listener],
        ))
    }

    fn from_listeners(
        runtime: Arc<ClientRuntime>,
        address: SocketAddr,
        policy: RoutingPolicy,
        listeners: Vec<TcpListener>,
    ) -> Self {
        let (shutdown, mut stopped) = watch::channel(false);
        let policy = Arc::new(policy);
        let task = tokio::spawn(async move {
            let mut tasks = Vec::with_capacity(listeners.len());
            for listener in listeners {
                let runtime = Arc::clone(&runtime);
                let policy = Arc::clone(&policy);
                let mut stopped = stopped.clone();
                tasks.push(tokio::spawn(async move {
                    loop {
                        tokio::select! {
                            changed = stopped.changed() => {
                                if changed.is_err() || *stopped.borrow() { break; }
                            }
                            accepted = listener.accept() => match accepted {
                                Ok((stream, _)) => {
                                    let runtime = Arc::clone(&runtime);
                                    let policy = Arc::clone(&policy);
                                    tokio::spawn(async move {
                                        let _ = serve(stream, runtime, policy).await;
                                    });
                                }
                                Err(_) => break,
                            },
                        }
                    }
                }));
            }
            let _ = stopped.changed().await;
            for task in tasks {
                task.abort();
            }
        });
        Self {
            address,
            shutdown,
            task,
        }
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

async fn serve(
    mut local: TcpStream,
    runtime: Arc<ClientRuntime>,
    policy: Arc<RoutingPolicy>,
) -> io::Result<()> {
    let request = read_request(&mut local).await?;
    let target = request.target;
    let domain = target.domain().map(str::to_owned);
    let target = target.resolve().await?;
    match policy.decide(RouteContext {
        domain: domain.as_deref(),
        destination: target.ip(),
        destination_port: target.port(),
    }) {
        RoutingAction::Direct => {
            let mut remote = TcpStream::connect(target).await?;
            write_success(&mut local, &request.protocol).await?;
            if let Some(head) = request.protocol.forward_head() {
                remote.write_all(head).await?;
            }
            relay_direct(local, remote).await
        }
        RoutingAction::Proxy => relay_proxy(local, runtime, target, request.protocol).await,
        RoutingAction::Reject => {
            write_rejected(&mut local, &request.protocol).await?;
            Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "proxy request rejected by routing policy",
            ))
        }
        RoutingAction::Node(_) => {
            write_rejected(&mut local, &request.protocol).await?;
            Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "explicit node routing requires client engine ABI v3",
            ))
        }
    }
}

async fn relay_proxy(
    mut local: TcpStream,
    runtime: Arc<ClientRuntime>,
    target: SocketAddr,
    protocol: ConnectProtocol,
) -> io::Result<()> {
    let stream = runtime.open_stream(target).await.map_err(runtime_error)?;
    let (generation, mut send, receive) = stream.into_parts();
    if !runtime.is_generation_online(generation) {
        return Err(io::Error::new(
            io::ErrorKind::NotConnected,
            "runtime generation ended",
        ));
    }
    write_success(&mut local, &protocol).await?;
    if let Some(head) = protocol.forward_head() {
        send.write_all(head).await.map_err(runtime_error)?;
    }
    relay(local, send, receive).await
}

async fn relay_direct(mut local: TcpStream, mut remote: TcpStream) -> io::Result<()> {
    tokio::io::copy_bidirectional(&mut local, &mut remote).await?;
    Ok(())
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

struct ProxyRequest {
    target: ProxyTarget,
    protocol: ConnectProtocol,
}

enum ConnectProtocol {
    HttpConnect,
    HttpForward(Vec<u8>),
    Socks5,
}

impl ConnectProtocol {
    fn forward_head(&self) -> Option<&[u8]> {
        match self {
            Self::HttpForward(head) => Some(head),
            Self::HttpConnect | Self::Socks5 => None,
        }
    }
}

async fn read_request<T>(local: &mut T) -> io::Result<ProxyRequest>
where
    T: AsyncRead + AsyncWrite + Unpin,
{
    let mut first = [0_u8; 1];
    local.read_exact(&mut first).await?;
    if first[0] == 5 {
        return read_socks5_target(local).await.map(|target| ProxyRequest {
            target,
            protocol: ConnectProtocol::Socks5,
        });
    }
    read_http_request(local, first[0]).await
}

async fn read_socks5_target<T>(local: &mut T) -> io::Result<ProxyTarget>
where
    T: AsyncRead + AsyncWrite + Unpin,
{
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
            ProxyTarget::Address(SocketAddr::new(IpAddr::V4(value.into()), 0))
        }
        4 => {
            let mut value = [0_u8; 16];
            local.read_exact(&mut value).await?;
            ProxyTarget::Address(SocketAddr::new(IpAddr::V6(value.into()), 0))
        }
        3 => {
            let mut length = [0_u8; 1];
            local.read_exact(&mut length).await?;
            let mut host = vec![0_u8; usize::from(length[0])];
            local.read_exact(&mut host).await?;
            ProxyTarget::Domain {
                host: String::from_utf8(host).map_err(|_| {
                    io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "proxy target hostname is not UTF-8",
                    )
                })?,
                port: 0,
            }
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
    let port = u16::from_be_bytes(port);
    match address {
        ProxyTarget::Address(address) => {
            Ok(ProxyTarget::Address(SocketAddr::new(address.ip(), port)))
        }
        ProxyTarget::Domain { host, .. } => ProxyTarget::from_host_port(host.as_bytes(), port),
    }
}

async fn read_http_request<T>(local: &mut T, first: u8) -> io::Result<ProxyRequest>
where
    T: AsyncRead + Unpin,
{
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
    if method.eq_ignore_ascii_case("CONNECT") {
        let version = parts
            .next()
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "missing HTTP version"))?;
        if !matches!(version, "HTTP/1.0" | "HTTP/1.1") || parts.next().is_some() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "invalid HTTP CONNECT request line",
            ));
        }
        return Ok(ProxyRequest {
            target: ProxyTarget::from_authority(authority)?,
            protocol: ConnectProtocol::HttpConnect,
        });
    }
    let forward = parse_forward_request(&request)?;
    Ok(ProxyRequest {
        target: forward.target,
        protocol: ConnectProtocol::HttpForward(forward.head),
    })
}

async fn write_success<T>(local: &mut T, protocol: &ConnectProtocol) -> io::Result<()>
where
    T: AsyncWrite + Unpin,
{
    match protocol {
        ConnectProtocol::HttpConnect => {
            local
                .write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n")
                .await
        }
        ConnectProtocol::Socks5 => local.write_all(&[5, 0, 0, 1, 0, 0, 0, 0, 0, 0]).await,
        ConnectProtocol::HttpForward(_) => Ok(()),
    }
}

async fn write_rejected<T>(local: &mut T, protocol: &ConnectProtocol) -> io::Result<()>
where
    T: AsyncWrite + Unpin,
{
    match protocol {
        ConnectProtocol::HttpConnect | ConnectProtocol::HttpForward(_) => {
            local
                .write_all(b"HTTP/1.1 403 Forbidden\r\nContent-Length: 0\r\n\r\n")
                .await
        }
        ConnectProtocol::Socks5 => local.write_all(&[5, 2, 0, 1, 0, 0, 0, 0, 0, 0]).await,
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

    #[tokio::test]
    async fn socks_domain_request_preserves_hostname_until_port_is_read() {
        let (mut client_stream, mut server) = tokio::io::duplex(128);
        let client = tokio::spawn(async move {
            client_stream.write_all(&[5, 1, 0]).await.expect("greeting");
            let mut response = [0_u8; 2];
            client_stream
                .read_exact(&mut response)
                .await
                .expect("greeting response");
            assert_eq!(response, [5, 0]);
            client_stream
                .write_all(&[5, 1, 0, 3, 11])
                .await
                .expect("request prefix");
            client_stream.write_all(b"example.com").await.expect("host");
            client_stream
                .write_all(&443_u16.to_be_bytes())
                .await
                .expect("port");
        });
        let request = read_request(&mut server).await.expect("target");
        assert!(matches!(request.protocol, ConnectProtocol::Socks5));
        assert_eq!(
            request.target,
            ProxyTarget::Domain {
                host: "example.com".into(),
                port: 443,
            }
        );
        client.await.expect("client task");
    }

    #[tokio::test]
    async fn reads_and_rewrites_absolute_form_http_request() {
        let request = b"POST http://example.com/upload HTTP/1.1\r\nProxy-Connection: keep-alive\r\nContent-Length: 3\r\n\r\nabc";
        let (mut client, mut server) = tokio::io::duplex(256);
        client.write_all(request).await.expect("write request");
        let parsed = read_request(&mut server).await.expect("request");
        assert_eq!(
            parsed.target,
            ProxyTarget::Domain {
                host: "example.com".into(),
                port: 80,
            }
        );
        let ConnectProtocol::HttpForward(head) = parsed.protocol else {
            panic!("expected HTTP forward request");
        };
        assert!(head.starts_with(b"POST /upload HTTP/1.1\r\n"));
        let mut body = [0_u8; 3];
        server.read_exact(&mut body).await.expect("body");
        assert_eq!(&body, b"abc");
    }

    #[tokio::test]
    async fn rejects_oversized_http_request_header() {
        let mut request = b"GET http://example.com/ HTTP/1.1\r\nX-Fill: ".to_vec();
        request.resize(MAX_HANDSHAKE_BYTES, b'a');
        let (mut client, mut server) = tokio::io::duplex(MAX_HANDSHAKE_BYTES);
        let writer = tokio::spawn(async move { client.write_all(&request).await });
        let error = match read_request(&mut server).await {
            Ok(_) => panic!("expected oversized header error"),
            Err(error) => error,
        };
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        writer.await.expect("writer task").expect("write request");
    }

    #[tokio::test]
    async fn rejected_http_forward_request_receives_forbidden_response() {
        let mut response = Vec::new();
        write_rejected(&mut response, &ConnectProtocol::HttpForward(Vec::new()))
            .await
            .expect("rejection");
        assert_eq!(
            response,
            b"HTTP/1.1 403 Forbidden\r\nContent-Length: 0\r\n\r\n"
        );
    }
}

//! Application-owned listener for an optional Forest cover service.
//!
//! TLS termination remains outside this listener. It accepts already decrypted
//! HTTP/1.1 connections and delegates all cover behavior to `velum-forest`.

use std::sync::Arc;

use tokio::{
    net::TcpListener,
    sync::{Semaphore, watch},
    task::JoinSet,
};
use velum_forest::{CoverServiceConfig, serve_cover_connection};

/// Binds the optional TCP cover-service endpoint.
pub async fn bind_cover_listener(
    address: std::net::SocketAddr,
) -> Result<TcpListener, std::io::Error> {
    TcpListener::bind(address).await
}

/// Serves cover requests until process shutdown. Admission is bounded before a
/// request head or upstream connection can consume per-connection resources.
pub async fn serve_cover_listener(
    listener: TcpListener,
    config: CoverServiceConfig,
    max_connections: usize,
    mut shutdown: watch::Receiver<bool>,
) {
    let slots = Arc::new(Semaphore::new(max_connections));
    let config = Arc::new(config);
    let mut connections = JoinSet::new();
    loop {
        tokio::select! {
            changed = shutdown.changed() => {
                if changed.is_err() || *shutdown.borrow() {
                    break;
                }
            }
            accepted = listener.accept() => {
                let Ok((mut connection, _)) = accepted else {
                    continue;
                };
                let Ok(slot) = Arc::clone(&slots).try_acquire_owned() else {
                    continue;
                };
                let config = Arc::clone(&config);
                connections.spawn(async move {
                    let _slot = slot;
                    let _ = serve_cover_connection(&mut connection, config.as_ref()).await;
                });
            }
            Some(_) = connections.join_next(), if !connections.is_empty() => {}
        }
    }
    connections.abort_all();
    while connections.join_next().await.is_some() {}
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpStream,
        time::timeout,
    };

    use super::*;

    #[tokio::test]
    async fn listener_proxies_an_http_request_to_its_configured_upstream() {
        let upstream = match TcpListener::bind("127.0.0.1:0").await {
            Ok(listener) => listener,
            Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => return,
            Err(error) => panic!("upstream listener: {error}"),
        };
        let upstream_address = upstream.local_addr().expect("upstream address");
        let upstream_task = tokio::spawn(async move {
            let (mut connection, _) = upstream.accept().await.expect("upstream connection");
            let mut request = [0; 128];
            let read = connection
                .read(&mut request)
                .await
                .expect("upstream request");
            assert!(request[..read].starts_with(b"GET /health HTTP/1.1\r\n"));
            connection
                .write_all(
                    b"HTTP/1.1 204 No Content\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                )
                .await
                .expect("upstream response");
        });
        let listener =
            match bind_cover_listener("127.0.0.1:0".parse().expect("cover address")).await {
                Ok(listener) => listener,
                Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => return,
                Err(error) => panic!("cover listener: {error}"),
            };
        let address = listener.local_addr().expect("cover address");
        let (stop, shutdown) = watch::channel(false);
        let cover_task = tokio::spawn(serve_cover_listener(
            listener,
            CoverServiceConfig {
                schema_version: 1,
                mode: velum_forest::CoverServiceMode::ReverseProxy {
                    upstream: upstream_address,
                },
                request_head_timeout: Duration::from_secs(1),
                upstream_timeout: Duration::from_secs(1),
            },
            1,
            shutdown,
        ));

        let mut client = TcpStream::connect(address).await.expect("cover connection");
        client
            .write_all(b"GET /health HTTP/1.1\r\nHost: cover.example\r\n\r\n")
            .await
            .expect("cover request");
        client.shutdown().await.expect("cover request close");
        let mut response = Vec::new();
        timeout(Duration::from_secs(2), client.read_to_end(&mut response))
            .await
            .expect("cover response timeout")
            .expect("cover response");
        assert!(response.starts_with(b"HTTP/1.1 204 No Content\r\n"));

        stop.send(true).expect("stop cover listener");
        cover_task.await.expect("cover listener task");
        upstream_task.await.expect("upstream task");
    }
}

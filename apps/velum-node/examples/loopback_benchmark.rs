//! Measures a direct or Velum-relayed QUIC echo flow on loopback.
//!
//! This is a diagnostic baseline, not a publishable network comparison. It
//! exercises the real listener, admission checks, and bidirectional relay.

use std::{error::Error, net::SocketAddr, sync::Arc, time::Instant};

use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    sync::{Mutex, mpsc, oneshot},
};
use velum_node::admin::Control;
use velum_node::{
    NoopRelayObserver, QuicRelayConfig, RelayAdmission, bind_quic_listener, encode_open,
    serve_quic_listener,
};
use velum_server::{
    AdmissionControl, Authenticator, DestinationPolicy, PrincipalCredential, PrincipalId,
    PrincipalQuota,
};

const SECRET: [u8; 32] = [7; 32];

#[derive(Clone, Copy)]
enum Mode {
    Direct,
    Velum,
}

impl Mode {
    fn name(self) -> &'static str {
        match self {
            Self::Direct => "direct",
            Self::Velum => "velum",
        }
    }
}

impl std::str::FromStr for Mode {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "direct" => Ok(Self::Direct),
            "velum" => Ok(Self::Velum),
            _ => Err(format!("unknown mode: {value}; expected direct or velum")),
        }
    }
}

struct BenchmarkServer {
    address: SocketAddr,
    target: Option<SocketAddr>,
    shutdown: Option<oneshot::Sender<()>>,
    task: tokio::task::JoinHandle<Result<(), Box<dyn Error + Send + Sync>>>,
    echo: Option<tokio::task::JoinHandle<Result<(), std::io::Error>>>,
}

impl BenchmarkServer {
    async fn stop(mut self) -> Result<(), Box<dyn Error>> {
        if let Some(shutdown) = self.shutdown.take() {
            shutdown.send(()).map_err(|_| "listener already stopped")?;
        }
        self.task
            .await?
            .map_err(|error| std::io::Error::other(error.to_string()))?;
        if let Some(echo) = self.echo {
            echo.await??;
        }
        Ok(())
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn Error>> {
    let (mode, payload_bytes, rounds, target_nodelay) = arguments()?;

    let certified = rcgen::generate_simple_self_signed(vec!["localhost".to_owned()])?;
    let certificate = rustls::pki_types::CertificateDer::from(certified.cert);
    let key = rustls::pki_types::PrivatePkcs8KeyDer::from(certified.signing_key.serialize_der());
    let server_config =
        quinn::ServerConfig::with_single_cert(vec![certificate.clone()], key.into())?;
    let server = start_server(mode, server_config, target_nodelay).await?;

    let mut roots = rustls::RootCertStore::empty();
    roots.add(certificate)?;
    let client_config = quinn::ClientConfig::with_root_certificates(Arc::new(roots))?;
    let mut client = quinn::Endpoint::client("127.0.0.1:0".parse()?)?;
    client.set_default_client_config(client_config);
    let connection_started = Instant::now();
    let connection = client.connect(server.address, "localhost")?.await?;
    let connection_millis = connection_started.elapsed().as_secs_f64() * 1_000.0;
    let stream_started = Instant::now();
    let (mut send, mut receive) = connection.open_bi().await?;
    let stream_millis = stream_started.elapsed().as_secs_f64() * 1_000.0;
    let control_millis = if let Some(target) = server.target {
        let control_started = Instant::now();
        let open_request = encode_open(&velum_node::OpenRequest {
            secret: SECRET.to_vec(),
            target,
        })
        .map_err(|_| "could not encode benchmark open request")?;
        send.write_all(&open_request).await?;
        control_started.elapsed().as_secs_f64() * 1_000.0
    } else {
        0.0
    };

    let payload = vec![0x5a; payload_bytes];
    let mut echoed = vec![0; payload_bytes];
    let started = Instant::now();
    let mut round_trip_millis = Vec::with_capacity(rounds);
    for _ in 0..rounds {
        let round_started = Instant::now();
        send.write_all(&payload).await?;
        send.flush().await?;
        receive.read_exact(&mut echoed).await?;
        if echoed != payload {
            return Err("echo payload mismatch".into());
        }
        round_trip_millis.push(round_started.elapsed().as_secs_f64() * 1_000.0);
    }
    let elapsed = started.elapsed();
    send.finish()?;
    match mode {
        Mode::Direct => {
            server.stop().await?;
            connection.close(0_u32.into(), b"benchmark complete");
            client.close(0_u32.into(), b"benchmark complete");
        }
        Mode::Velum => {
            connection.close(0_u32.into(), b"benchmark complete");
            client.close(0_u32.into(), b"benchmark complete");
            server.stop().await?;
        }
    }

    let bytes = payload_bytes
        .checked_mul(rounds)
        .ok_or("byte count overflow")?;
    let elapsed_seconds = elapsed.as_secs_f64();
    round_trip_millis.sort_by(f64::total_cmp);
    println!(
        "{{\"mode\":\"{}\",\"target_nodelay\":{target_nodelay},\"payload_bytes\":{payload_bytes},\"rounds\":{rounds},\"connection_ms\":{connection_millis:.3},\"stream_open_ms\":{stream_millis:.3},\"control_write_ms\":{control_millis:.3},\"elapsed_ms\":{:.3},\"round_trip_p50_ms\":{:.3},\"round_trip_p95_ms\":{:.3},\"round_trip_max_ms\":{:.3},\"application_mbit_s\":{:.3}}}",
        mode.name(),
        elapsed_seconds * 1_000.0,
        percentile(&round_trip_millis, 50),
        percentile(&round_trip_millis, 95),
        round_trip_millis.iter().copied().fold(0.0, f64::max),
        bytes as f64 * 8.0 / elapsed_seconds / 1_000_000.0,
    );
    Ok(())
}

fn percentile(values: &[f64], percent: usize) -> f64 {
    let index = (values.len() * percent).div_ceil(100) - 1;
    values[index]
}

fn arguments() -> Result<(Mode, usize, usize, bool), Box<dyn Error>> {
    let mut mode = Mode::Velum;
    let mut payload_bytes = 65_536;
    let mut rounds = 100;
    let mut target_nodelay = false;
    let mut arguments = std::env::args().skip(1);
    while let Some(argument) = arguments.next() {
        let value = arguments.next().ok_or("missing option value")?;
        match argument.as_str() {
            "--mode" => mode = value.parse()?,
            "--payload-bytes" => payload_bytes = value.parse()?,
            "--rounds" => rounds = value.parse()?,
            "--target-nodelay" => target_nodelay = value.parse()?,
            _ => return Err(format!("unknown option: {argument}").into()),
        }
    }
    if payload_bytes == 0 || rounds == 0 {
        return Err("payload bytes and rounds must be positive".into());
    }
    Ok((mode, payload_bytes, rounds, target_nodelay))
}

fn admission(target: SocketAddr) -> RelayAdmission {
    RelayAdmission {
        authenticator: Arc::new(
            Authenticator::new([PrincipalCredential::new(PrincipalId(1), SECRET.to_vec())
                .expect("constant credential")])
            .expect("constant authenticator"),
        ),
        destinations: Arc::new(DestinationPolicy::new([target])),
        quotas: Arc::new(Mutex::new(AdmissionControl::new(PrincipalQuota {
            max_sessions: 1,
            max_flows_per_session: 1,
        }))),
    }
}

async fn start_server(
    mode: Mode,
    server_config: quinn::ServerConfig,
    target_nodelay: bool,
) -> Result<BenchmarkServer, Box<dyn Error>> {
    match mode {
        Mode::Direct => {
            let endpoint = quinn::Endpoint::server(server_config, "127.0.0.1:0".parse()?)?;
            let address = endpoint.local_addr()?;
            let task = tokio::spawn(async move { serve_direct_echo(endpoint).await });
            Ok(BenchmarkServer {
                address,
                target: None,
                shutdown: None,
                task,
                echo: None,
            })
        }
        Mode::Velum => {
            let target = TcpListener::bind("127.0.0.1:0").await?;
            let target_address = target.local_addr()?;
            let echo = tokio::spawn(serve_echo(target, target_nodelay));
            let endpoint = bind_quic_listener("127.0.0.1:0".parse()?, server_config)?;
            let address = endpoint.local_addr()?;
            let (shutdown_sender, shutdown) = oneshot::channel();
            let (controls, control_requests) = mpsc::channel(1);
            tokio::spawn(async move {
                let _ = shutdown.await;
                let _ = controls.send(Control::Shutdown).await;
            });
            let listener = tokio::spawn(async move {
                serve_quic_listener(
                    endpoint,
                    admission(target_address),
                    QuicRelayConfig::default(),
                    Arc::new(NoopRelayObserver),
                    Arc::new(velum_node::admin::RuntimeStatus::default()),
                    control_requests,
                    || Err("benchmark listener does not support reload".into()),
                )
                .await
                .map_err(|error| error.into())
            });
            Ok(BenchmarkServer {
                address,
                target: Some(target_address),
                shutdown: Some(shutdown_sender),
                task: listener,
                echo: Some(echo),
            })
        }
    }
}

async fn serve_direct_echo(endpoint: quinn::Endpoint) -> Result<(), Box<dyn Error + Send + Sync>> {
    let incoming = endpoint.accept().await.ok_or("direct endpoint closed")?;
    let connection = incoming.await?;
    let (mut send, mut receive) = connection.accept_bi().await?;
    let mut buffer = [0; 16 * 1024];
    loop {
        let read = receive.read(&mut buffer).await?;
        let Some(read) = read else {
            send.finish()?;
            return Ok(());
        };
        send.write_all(&buffer[..read]).await?;
        send.flush().await?;
    }
}

async fn serve_echo(listener: TcpListener, nodelay: bool) -> Result<(), std::io::Error> {
    let (mut connection, _) = listener.accept().await?;
    connection.set_nodelay(nodelay)?;
    let mut buffer = [0; 16 * 1024];
    loop {
        let read = connection.read(&mut buffer).await?;
        if read == 0 {
            return Ok(());
        }
        connection.write_all(&buffer[..read]).await?;
    }
}

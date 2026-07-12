//! Local-only operator control socket.

use std::{
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicU8, AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};

const REQUEST_TIMEOUT: Duration = Duration::from_secs(5);
const MAX_REQUEST_BYTES: usize = 32;
const RUNNING: u8 = 0;
const DRAINING: u8 = 1;
const STOPPING: u8 = 2;

use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{UnixListener, UnixStream},
    sync::{mpsc, oneshot},
    task::JoinHandle,
};

#[derive(Debug)]
pub enum Control {
    Drain,
    Reload(oneshot::Sender<Result<(), String>>),
    Shutdown,
}

#[derive(Default)]
pub struct RuntimeStatus {
    phase: AtomicU8,
    connections: AtomicUsize,
    flows: AtomicUsize,
}

impl RuntimeStatus {
    pub fn connection_opened(&self) {
        self.connections.fetch_add(1, Ordering::Relaxed);
    }
    pub fn connection_closed(&self) {
        self.connections.fetch_sub(1, Ordering::Relaxed);
    }
    pub fn flow_opened(&self) {
        self.flows.fetch_add(1, Ordering::Relaxed);
    }
    pub fn flow_closed(&self) {
        self.flows.fetch_sub(1, Ordering::Relaxed);
    }

    fn set_phase(&self, phase: u8) {
        self.phase.store(phase, Ordering::Relaxed);
    }

    fn render(&self, bind: &str, started: Instant, json: bool) -> String {
        let state = match self.phase.load(Ordering::Relaxed) {
            RUNNING => "running",
            DRAINING => "draining",
            STOPPING => "stopping",
            _ => "running",
        };
        let uptime = started.elapsed().as_secs();
        let connections = self.connections.load(Ordering::Relaxed);
        let flows = self.flows.load(Ordering::Relaxed);
        if json {
            format!(
                "{{\"state\":\"{state}\",\"listener\":\"{bind}\",\"uptime_secs\":{uptime},\"connections\":{connections},\"flows\":{flows}}}\n"
            )
        } else {
            format!(
                "{state}\nlistener={bind}\nuptime_secs={uptime}\nconnections={connections}\nflows={flows}\n"
            )
        }
    }
}

pub struct Server {
    task: JoinHandle<()>,
    path: PathBuf,
}

impl Server {
    pub fn stop(self) {
        self.task.abort();
        let _ = std::fs::remove_file(self.path);
    }
}

pub fn spawn(
    path: PathBuf,
    bind: String,
    controls: mpsc::Sender<Control>,
    status: Arc<RuntimeStatus>,
) -> Result<Server, String> {
    let parent = path
        .parent()
        .ok_or("admin socket path has no parent directory")?;
    let parent_existed = parent.exists();
    std::fs::create_dir_all(parent).map_err(|error| {
        format!(
            "cannot create admin socket directory {}: {error}",
            parent.display()
        )
    })?;
    #[cfg(unix)]
    {
        if parent_existed {
            verify_private_directory(parent)?;
        } else {
            restrict_directory(parent)?;
        }
    }
    if path.exists() {
        #[cfg(unix)]
        verify_socket_path(&path)?;
        std::fs::remove_file(&path)
            .map_err(|error| format!("cannot replace admin socket {}: {error}", path.display()))?;
    }
    let listener = UnixListener::bind(&path)
        .map_err(|error| format!("cannot bind admin socket {}: {error}", path.display()))?;
    #[cfg(unix)]
    restrict_socket(&path)?;
    let started = Instant::now();
    let task = tokio::spawn(async move {
        while let Ok((stream, _)) = listener.accept().await {
            let controls = controls.clone();
            let bind = bind.clone();
            let status = Arc::clone(&status);
            tokio::spawn(async move {
                let _ = handle(stream, bind, started, controls, status).await;
            });
        }
    });
    Ok(Server { task, path })
}

pub async fn request(path: &Path, request: &str) -> Result<String, String> {
    let response = tokio::time::timeout(REQUEST_TIMEOUT, request_inner(path, request))
        .await
        .map_err(|_| format!("admin request to {} timed out", path.display()))??;
    if let Some(error) = response.strip_prefix("error=") {
        return Err(error.trim().to_owned());
    }
    Ok(response)
}

async fn request_inner(path: &Path, request: &str) -> Result<String, String> {
    let mut stream = UnixStream::connect(path)
        .await
        .map_err(|error| format!("cannot connect to admin socket {}: {error}", path.display()))?;
    stream
        .write_all(request.as_bytes())
        .await
        .map_err(|error| format!("cannot write admin request: {error}"))?;
    stream
        .write_all(b"\n")
        .await
        .map_err(|error| format!("cannot finish admin request: {error}"))?;
    let mut response = Vec::new();
    stream
        .read_to_end(&mut response)
        .await
        .map_err(|error| format!("cannot read admin response: {error}"))?;
    String::from_utf8(response).map_err(|_| "admin response is not UTF-8".into())
}

async fn handle(
    mut stream: UnixStream,
    bind: String,
    started: Instant,
    controls: mpsc::Sender<Control>,
    status: Arc<RuntimeStatus>,
) -> Result<(), String> {
    let request = read_request(&mut stream).await;
    let response = match request.as_deref().unwrap_or("") {
        "status" => status.render(&bind, started, false),
        "status json" => status.render(&bind, started, true),
        "drain" => match controls.send(Control::Drain).await {
            Ok(()) => {
                status.set_phase(DRAINING);
                "draining\n".into()
            }
            Err(_) => "stopping\n".into(),
        },
        "reload" => reload(controls).await,
        "shutdown" => match controls.send(Control::Shutdown).await {
            Ok(()) => {
                status.set_phase(STOPPING);
                "stopping\n".into()
            }
            Err(_) => "stopping\n".into(),
        },
        _ if request.is_err() => "error=invalid request\n".into(),
        _ => "error=unknown command\n".into(),
    };
    stream
        .write_all(response.as_bytes())
        .await
        .map_err(|error| format!("cannot write admin response: {error}"))
}

async fn reload(controls: mpsc::Sender<Control>) -> String {
    let (completion, result) = oneshot::channel();
    if controls.send(Control::Reload(completion)).await.is_err() {
        return "error=reload unavailable\n".into();
    }
    match tokio::time::timeout(REQUEST_TIMEOUT, result).await {
        Ok(Ok(Ok(()))) => "reloaded\n".into(),
        _ => "error=reload failed\n".into(),
    }
}

async fn read_request(stream: &mut UnixStream) -> Result<String, String> {
    tokio::time::timeout(REQUEST_TIMEOUT, async {
        let mut request = Vec::with_capacity(MAX_REQUEST_BYTES);
        loop {
            let mut byte = [0_u8; 1];
            let read = stream
                .read(&mut byte)
                .await
                .map_err(|error| format!("cannot read admin request: {error}"))?;
            if read == 0 {
                return Err("admin request ended before newline".into());
            }
            if byte[0] == b'\n' {
                return String::from_utf8(request).map_err(|_| "admin request is not UTF-8".into());
            }
            if request.len() == MAX_REQUEST_BYTES {
                return Err("admin request is too long".into());
            }
            request.push(byte[0]);
        }
    })
    .await
    .map_err(|_| "admin request timed out".to_owned())?
}

#[cfg(unix)]
fn restrict_directory(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700)).map_err(|error| {
        format!(
            "cannot restrict admin directory {}: {error}",
            path.display()
        )
    })
}

#[cfg(unix)]
fn verify_private_directory(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    let mode = std::fs::metadata(path)
        .map_err(|error| format!("cannot inspect admin directory {}: {error}", path.display()))?
        .permissions()
        .mode();
    if mode & 0o077 != 0 {
        return Err(format!(
            "admin directory {} must not be group or world accessible",
            path.display()
        ));
    }
    Ok(())
}

#[cfg(unix)]
fn verify_socket_path(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::FileTypeExt;
    let kind = std::fs::symlink_metadata(path)
        .map_err(|error| format!("cannot inspect admin socket {}: {error}", path.display()))?
        .file_type();
    if !kind.is_socket() {
        return Err(format!(
            "refusing to replace non-socket admin path {}",
            path.display()
        ));
    }
    Ok(())
}

#[cfg(unix)]
fn restrict_socket(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
        .map_err(|error| format!("cannot restrict admin socket {}: {error}", path.display()))
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;
    use tokio::net::UnixStream;

    #[tokio::test]
    async fn status_and_shutdown_are_local_socket_operations() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let directory = std::env::temp_dir().join(format!("velum-admin-{unique}"));
        let path = directory.join("admin.sock");
        let (sender, mut receiver) = mpsc::channel(1);
        let Ok(server) = spawn(
            path.clone(),
            "127.0.0.1:4433".into(),
            sender,
            Arc::new(RuntimeStatus::default()),
        ) else {
            // Some constrained test sandboxes forbid Unix-domain socket binding.
            return;
        };
        assert!(
            request(&path, "status")
                .await
                .expect("status")
                .contains("running")
        );
        assert!(
            request(&path, "status json")
                .await
                .expect("JSON status")
                .contains("\"connections\":0")
        );
        assert_eq!(
            request(&path, "shutdown").await.expect("shutdown"),
            "stopping\n"
        );
        assert!(matches!(receiver.recv().await, Some(Control::Shutdown)));
        server.stop();
        let _ = std::fs::remove_dir(directory);
    }

    #[tokio::test]
    async fn fragmented_commands_are_read_to_the_newline_boundary() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let directory = std::env::temp_dir().join(format!("velum-admin-{unique}"));
        let path = directory.join("admin.sock");
        let (sender, mut receiver) = mpsc::channel(1);
        let Ok(server) = spawn(
            path.clone(),
            "127.0.0.1:4433".into(),
            sender,
            Arc::new(RuntimeStatus::default()),
        ) else {
            return;
        };

        let mut client = UnixStream::connect(&path).await.expect("connect");
        client.write_all(b"shut").await.expect("first fragment");
        client.write_all(b"down\n").await.expect("second fragment");
        let mut response = Vec::new();
        client.read_to_end(&mut response).await.expect("response");
        assert_eq!(response, b"stopping\n");
        assert!(matches!(receiver.recv().await, Some(Control::Shutdown)));
        server.stop();
        let _ = std::fs::remove_dir(directory);
    }

    #[tokio::test]
    async fn reload_reports_listener_rejection_to_the_caller() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let directory = std::env::temp_dir().join(format!("velum-admin-{unique}"));
        let path = directory.join("admin.sock");
        let (sender, mut receiver) = mpsc::channel(1);
        let Ok(server) = spawn(
            path.clone(),
            "127.0.0.1:4433".into(),
            sender,
            Arc::new(RuntimeStatus::default()),
        ) else {
            return;
        };
        let responder = tokio::spawn(async move {
            if let Some(Control::Reload(completion)) = receiver.recv().await {
                let _ = completion.send(Err("invalid configuration".into()));
            }
        });

        assert_eq!(request(&path, "reload").await, Err("reload failed".into()));
        responder.await.expect("responder");
        server.stop();
        let _ = std::fs::remove_dir(directory);
    }
}

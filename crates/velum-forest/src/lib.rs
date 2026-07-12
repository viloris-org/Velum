//! Forest Native owns cover-service behavior and traffic-profile selection.
//!
//! It deliberately has no dependency on Velum session or authentication state.
//! A caller may disable Forest traffic behavior after a profile failure while
//! leaving the cover service available.

use std::{
    collections::BTreeMap,
    net::SocketAddr,
    time::{Duration, SystemTime},
};

use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
    time::timeout,
};

const MAX_REQUEST_HEAD_BYTES: usize = 16 * 1024;

/// Versioned configuration for a cover endpoint. TLS termination and listener
/// ownership stay at the application boundary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CoverServiceConfig {
    pub schema_version: u16,
    pub mode: CoverServiceMode,
    pub request_head_timeout: Duration,
    pub upstream_timeout: Duration,
}

impl CoverServiceConfig {
    pub fn validate(&self) -> Result<(), CoverConfigError> {
        if self.schema_version != 1 {
            return Err(CoverConfigError::UnsupportedSchema);
        }
        if self.upstream_timeout.is_zero() {
            return Err(CoverConfigError::ZeroUpstreamTimeout);
        }
        if self.request_head_timeout.is_zero() {
            return Err(CoverConfigError::ZeroRequestHeadTimeout);
        }
        self.mode.validate()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CoverServiceMode {
    Static(StaticService),
    ReverseProxy { upstream: SocketAddr },
}

impl CoverServiceMode {
    fn validate(&self) -> Result<(), CoverConfigError> {
        match self {
            Self::Static(service) => service.validate(),
            Self::ReverseProxy { .. } => Ok(()),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StaticService {
    pub assets: BTreeMap<String, StaticAsset>,
}

impl StaticService {
    fn validate(&self) -> Result<(), CoverConfigError> {
        if self.assets.is_empty()
            || self.assets.keys().any(|path| !valid_path(path))
            || self
                .assets
                .values()
                .any(|asset| !valid_content_type(&asset.content_type))
        {
            return Err(CoverConfigError::InvalidStaticService);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StaticAsset {
    pub content_type: String,
    pub body: Vec<u8>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CoverConfigError {
    UnsupportedSchema,
    ZeroRequestHeadTimeout,
    ZeroUpstreamTimeout,
    InvalidStaticService,
}

/// Serves a normal HTTP/1.1 static response or relays a normal HTTP/1.1
/// request to the configured reverse-proxy upstream. It emits only cover HTTP
/// responses; no Velum-specific status, header, or body is generated here.
pub async fn serve_cover_connection(
    client: &mut TcpStream,
    config: &CoverServiceConfig,
) -> Result<(), CoverServiceError> {
    config
        .validate()
        .map_err(|_| CoverServiceError::InvalidConfig)?;
    let head = timeout(config.request_head_timeout, read_request_head(client))
        .await
        .map_err(|_| CoverServiceError::RequestTimedOut)??;
    match &config.mode {
        CoverServiceMode::Static(service) => serve_static(client, service, &head).await,
        CoverServiceMode::ReverseProxy { upstream } => {
            let mut upstream = timeout(config.upstream_timeout, TcpStream::connect(upstream))
                .await
                .map_err(|_| CoverServiceError::UpstreamUnavailable)?
                .map_err(|_| CoverServiceError::UpstreamUnavailable)?;
            upstream
                .write_all(&head)
                .await
                .map_err(|_| CoverServiceError::Transport)?;
            tokio::io::copy_bidirectional(client, &mut upstream)
                .await
                .map_err(|_| CoverServiceError::Transport)?;
            Ok(())
        }
    }
}

async fn read_request_head(client: &mut TcpStream) -> Result<Vec<u8>, CoverServiceError> {
    let mut head = Vec::with_capacity(512);
    while head.len() < MAX_REQUEST_HEAD_BYTES {
        let mut byte = [0; 1];
        client
            .read_exact(&mut byte)
            .await
            .map_err(|_| CoverServiceError::Transport)?;
        head.push(byte[0]);
        if head.ends_with(b"\r\n\r\n") {
            return Ok(head);
        }
    }
    Err(CoverServiceError::MalformedRequest)
}

async fn serve_static(
    client: &mut TcpStream,
    service: &StaticService,
    head: &[u8],
) -> Result<(), CoverServiceError> {
    let path = request_path(head).ok_or(CoverServiceError::MalformedRequest)?;
    let missing = StaticAsset {
        content_type: "text/plain; charset=utf-8".into(),
        body: b"Not Found\n".to_vec(),
    };
    let (status, asset) = match service.assets.get(path) {
        Some(asset) => ("200 OK", asset),
        None => ("404 Not Found", &missing),
    };
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        asset.content_type,
        asset.body.len(),
    );
    client
        .write_all(response.as_bytes())
        .await
        .map_err(|_| CoverServiceError::Transport)?;
    if request_method(head) != Some("HEAD") {
        client
            .write_all(&asset.body)
            .await
            .map_err(|_| CoverServiceError::Transport)?;
    }
    Ok(())
}

fn request_path(head: &[u8]) -> Option<&str> {
    let first_line = std::str::from_utf8(head).ok()?.split("\r\n").next()?;
    let mut fields = first_line.split_ascii_whitespace();
    let method = fields.next()?;
    let path = fields.next()?;
    (method == "GET" || method == "HEAD").then_some(path)
}

fn request_method(head: &[u8]) -> Option<&str> {
    std::str::from_utf8(head)
        .ok()?
        .split("\r\n")
        .next()?
        .split_ascii_whitespace()
        .next()
}

fn valid_path(path: &str) -> bool {
    path.starts_with('/') && !path.contains("..") && !path.contains('\r') && !path.contains('\n')
}

fn valid_content_type(content_type: &str) -> bool {
    !content_type.is_empty() && !content_type.bytes().any(|byte| byte.is_ascii_control())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CoverServiceError {
    InvalidConfig,
    MalformedRequest,
    RequestTimedOut,
    UpstreamUnavailable,
    Transport,
}

/// An immutable, versioned profile. It selects carrier-write presentation only;
/// it cannot express frame, authentication, flow, epoch, or acknowledgement
/// changes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TrafficProfile {
    pub schema_version: u16,
    pub identifier: String,
    pub digest: [u8; 32],
    pub expires_at: SystemTime,
    pub write_size_classes: Vec<u16>,
}

impl TrafficProfile {
    pub fn validate(&self, now: SystemTime) -> Result<(), ProfileError> {
        if self.schema_version != 1 {
            return Err(ProfileError::UnsupportedSchema);
        }
        if self.identifier.is_empty()
            || self.identifier.len() > 64
            || self.write_size_classes.is_empty()
        {
            return Err(ProfileError::InvalidProfile);
        }
        if self
            .write_size_classes
            .windows(2)
            .any(|sizes| sizes[0] >= sizes[1])
        {
            return Err(ProfileError::InvalidProfile);
        }
        if self.expires_at <= now {
            return Err(ProfileError::Expired);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProfileUpdate {
    pub profile: TrafficProfile,
    pub proof: Vec<u8>,
}

/// The application supplies authentication. Forest accepts an update only when
/// its caller has verified that proof against the update's canonical bytes.
pub trait ProfileUpdateVerifier: Send + Sync {
    fn verify(&self, update: &ProfileUpdate) -> bool;
}

#[derive(Clone, Debug)]
pub struct ProfileStore {
    active: TrafficProfile,
}

impl ProfileStore {
    pub fn new(active: TrafficProfile, now: SystemTime) -> Result<Self, ProfileError> {
        active.validate(now)?;
        Ok(Self { active })
    }

    pub fn active(&self) -> &TrafficProfile {
        &self.active
    }

    pub fn rotate(
        &mut self,
        update: ProfileUpdate,
        verifier: &dyn ProfileUpdateVerifier,
        now: SystemTime,
    ) -> Result<(), ProfileError> {
        if !verifier.verify(&update) {
            return Err(ProfileError::AuthenticationRejected);
        }
        update.profile.validate(now)?;
        if update.profile.identifier == self.active.identifier
            || update.profile.expires_at <= self.active.expires_at
        {
            return Err(ProfileError::InvalidRotation);
        }
        self.active = update.profile;
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProfileError {
    UnsupportedSchema,
    InvalidProfile,
    Expired,
    AuthenticationRejected,
    InvalidRotation,
}

/// Kill switch for traffic-profile behavior. It leaves cover-service routing
/// untouched and never owns session delivery state.
#[derive(Clone, Debug)]
pub struct ForestRuntime {
    profiles: ProfileStore,
    traffic_enabled: bool,
}

impl ForestRuntime {
    pub fn new(profiles: ProfileStore) -> Self {
        Self {
            profiles,
            traffic_enabled: true,
        }
    }

    pub fn disable_after_failure(&mut self) {
        self.traffic_enabled = false;
    }

    pub fn enable(&mut self) {
        self.traffic_enabled = true;
    }

    pub fn active_profile(&self, now: SystemTime) -> Option<&TrafficProfile> {
        (self.traffic_enabled && self.profiles.active().expires_at > now)
            .then_some(self.profiles.active())
    }

    pub fn profiles_mut(&mut self) -> &mut ProfileStore {
        &mut self.profiles
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use tokio::net::TcpListener;

    fn profile(identifier: &str, expiry_offset: u64) -> TrafficProfile {
        TrafficProfile {
            schema_version: 1,
            identifier: identifier.into(),
            digest: [7; 32],
            expires_at: SystemTime::UNIX_EPOCH + Duration::from_secs(expiry_offset),
            write_size_classes: vec![256, 512, 1024],
        }
    }

    struct Accept;
    impl ProfileUpdateVerifier for Accept {
        fn verify(&self, _: &ProfileUpdate) -> bool {
            true
        }
    }
    struct Reject;
    impl ProfileUpdateVerifier for Reject {
        fn verify(&self, _: &ProfileUpdate) -> bool {
            false
        }
    }

    async fn loopback_listener() -> Option<TcpListener> {
        match TcpListener::bind("127.0.0.1:0").await {
            Ok(listener) => Some(listener),
            Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => None,
            Err(error) => panic!("listener: {error}"),
        }
    }

    fn static_config(request_head_timeout: Duration) -> CoverServiceConfig {
        CoverServiceConfig {
            schema_version: 1,
            mode: CoverServiceMode::Static(StaticService {
                assets: BTreeMap::from([(
                    "/".into(),
                    StaticAsset {
                        content_type: "text/plain".into(),
                        body: b"cover service\n".to_vec(),
                    },
                )]),
            }),
            request_head_timeout,
            upstream_timeout: Duration::from_secs(1),
        }
    }

    async fn run_cover_probe(config: CoverServiceConfig, request: &[u8]) -> Option<Vec<u8>> {
        let listener = loopback_listener().await?;
        let address = listener.local_addr().expect("address");
        let server = tokio::spawn(async move {
            let (mut connection, _) = listener.accept().await.expect("accept");
            let _ = serve_cover_connection(&mut connection, &config).await;
        });
        let mut client = TcpStream::connect(address).await.expect("connect");
        client.write_all(request).await.expect("request");
        let mut response = Vec::new();
        client.read_to_end(&mut response).await.expect("response");
        server.await.expect("server");
        Some(response)
    }

    #[test]
    fn profile_rotation_requires_authentication_and_a_later_expiry() {
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(1);
        let mut store = ProfileStore::new(profile("bootstrap", 100), now).expect("profile");
        let update = ProfileUpdate {
            profile: profile("rotated", 200),
            proof: vec![1],
        };
        assert_eq!(
            store.rotate(update.clone(), &Reject, now),
            Err(ProfileError::AuthenticationRejected)
        );
        assert!(store.rotate(update, &Accept, now).is_ok());
        assert_eq!(store.active().identifier, "rotated");
    }

    #[test]
    fn kill_switch_removes_only_traffic_profile_selection() {
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(1);
        let store = ProfileStore::new(profile("bootstrap", 100), now).expect("profile");
        let mut runtime = ForestRuntime::new(store);
        assert!(runtime.active_profile(now).is_some());
        runtime.disable_after_failure();
        assert!(runtime.active_profile(now).is_none());
        runtime.enable();
        assert_eq!(
            runtime.active_profile(now).expect("profile").identifier,
            "bootstrap"
        );
    }

    #[test]
    fn static_service_configuration_rejects_path_traversal() {
        let service = StaticService {
            assets: BTreeMap::from([(
                "../private".into(),
                StaticAsset {
                    content_type: "text/plain".into(),
                    body: vec![],
                },
            )]),
        };
        assert_eq!(
            service.validate(),
            Err(CoverConfigError::InvalidStaticService)
        );
    }

    #[tokio::test]
    async fn static_cover_service_remains_useful_without_forest_traffic() {
        let Some(listener) = loopback_listener().await else {
            return;
        };
        let config = Arc::new(static_config(Duration::from_secs(1)));
        let address = listener.local_addr().expect("address");
        let server = tokio::spawn(async move {
            let (mut connection, _) = listener.accept().await.expect("accept");
            serve_cover_connection(&mut connection, config.as_ref())
                .await
                .expect("serve");
        });

        let mut client = TcpStream::connect(address).await.expect("connect");
        client
            .write_all(b"GET / HTTP/1.1\r\nHost: cover.example\r\n\r\n")
            .await
            .expect("request");
        let mut response = Vec::new();
        client.read_to_end(&mut response).await.expect("response");
        server.await.expect("server");
        assert!(response.starts_with(b"HTTP/1.1 200 OK\r\n"));
        assert!(response.ends_with(b"cover service\n"));
    }

    #[tokio::test]
    async fn differential_pre_auth_probes_have_no_forest_specific_response() {
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(1);
        let profiles = ProfileStore::new(profile("bootstrap", 100), now).expect("profile");
        let mut disabled = ForestRuntime::new(profiles);
        disabled.disable_after_failure();
        assert!(disabled.active_profile(now).is_none());

        let probes: [&[u8]; 5] = [
            b"GET / HTTP/1.1\r\nHost: cover.example\r\n\r\n",
            b"POST / HTTP/1.1\r\nHost: cover.example\r\n\r\n",
            b"GET / HTTP/1.1\r\nHost: cover.example\r\n",
            b"GET / HTTP/1.1\r\nHost: cover.example\r\n\r\n",
            b"\xff\xfe\r\n\r\n",
        ];
        for probe in probes {
            let enabled = run_cover_probe(static_config(Duration::from_millis(10)), probe).await;
            let disabled = run_cover_probe(static_config(Duration::from_millis(10)), probe).await;
            let Some(enabled) = enabled else {
                return;
            };
            let Some(disabled) = disabled else {
                return;
            };
            assert_eq!(enabled, disabled, "probe response differs");
        }
    }

    #[tokio::test]
    async fn reverse_proxy_forwards_cover_request_and_response() {
        let Some(upstream_listener) = loopback_listener().await else {
            return;
        };
        let upstream_address = upstream_listener.local_addr().expect("upstream address");
        let upstream = tokio::spawn(async move {
            let (mut connection, _) = upstream_listener.accept().await.expect("upstream accept");
            let request = read_request_head(&mut connection)
                .await
                .expect("upstream request");
            assert!(request.starts_with(b"GET /health HTTP/1.1\r\n"));
            connection
                .write_all(
                    b"HTTP/1.1 204 No Content\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                )
                .await
                .expect("upstream response");
        });
        let Some(listener) = loopback_listener().await else {
            return;
        };
        let address = listener.local_addr().expect("proxy address");
        let server = tokio::spawn(async move {
            let (mut connection, _) = listener.accept().await.expect("proxy accept");
            serve_cover_connection(
                &mut connection,
                &CoverServiceConfig {
                    schema_version: 1,
                    mode: CoverServiceMode::ReverseProxy {
                        upstream: upstream_address,
                    },
                    request_head_timeout: Duration::from_secs(1),
                    upstream_timeout: Duration::from_secs(1),
                },
            )
            .await
            .expect("proxy serve");
        });

        let mut client = TcpStream::connect(address).await.expect("proxy connect");
        client
            .write_all(b"GET /health HTTP/1.1\r\nHost: cover.example\r\n\r\n")
            .await
            .expect("proxy request");
        let mut response = Vec::new();
        client
            .read_to_end(&mut response)
            .await
            .expect("proxy response");
        server.await.expect("proxy server");
        upstream.await.expect("upstream server");
        assert!(response.starts_with(b"HTTP/1.1 204 No Content\r\n"));
    }

    #[test]
    fn expired_profiles_are_not_selected_after_reenabling_traffic() {
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(1);
        let store = ProfileStore::new(profile("bootstrap", 2), now).expect("profile");
        let mut runtime = ForestRuntime::new(store);
        runtime.disable_after_failure();
        runtime.enable();
        assert!(
            runtime
                .active_profile(SystemTime::UNIX_EPOCH + Duration::from_secs(2))
                .is_none()
        );
    }

    #[test]
    fn static_content_type_rejects_response_splitting() {
        let service = StaticService {
            assets: BTreeMap::from([(
                "/".into(),
                StaticAsset {
                    content_type: "text/plain\r\nX-Injected: yes".into(),
                    body: vec![],
                },
            )]),
        };
        assert_eq!(
            service.validate(),
            Err(CoverConfigError::InvalidStaticService)
        );
    }
}

//! Versioned operator configuration and secret-file loading.

use std::{
    fs::{self, File},
    io::BufReader,
    net::SocketAddr,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use rustls::pki_types::{CertificateDer, PrivateKeyDer, pem::PemObject};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use velum_server::{
    AdmissionControl, Authenticator, DestinationPolicy, PrincipalCredential, PrincipalId,
    PrincipalQuota,
};
use x509_parser::parse_x509_certificate;

use crate::{QuicRelayConfig, RelayAdmission};

const VERSION: u16 = 1;
const MAX_SECRET_BYTES: usize = 128;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Config {
    pub version: u16,
    pub listener: ListenerConfig,
    pub admin: AdminConfig,
    pub credentials: Vec<CredentialConfig>,
    pub allowed_targets: Vec<String>,
    pub limits: Limits,
    #[serde(default)]
    pub acme: Option<AcmeConfig>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ListenerConfig {
    pub bind: String,
    pub certificate: PathBuf,
    pub private_key: PathBuf,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AdminConfig {
    pub socket: PathBuf,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CredentialConfig {
    pub id: u64,
    pub secret_file: PathBuf,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Limits {
    pub max_sessions: usize,
    pub max_flows_per_session: usize,
    pub max_connections: usize,
    pub max_streams_per_connection: usize,
    pub connect_timeout_secs: u64,
    pub control_timeout_secs: u64,
    pub shutdown_timeout_secs: u64,
}

/// Non-secret ACME policy. DNS-provider credentials stay in the environment
/// consumed by Lego and must never be written to this configuration file.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AcmeConfig {
    pub email: String,
    pub domains: Vec<String>,
    pub dns_provider: String,
    pub directory_url: String,
    pub state_dir: PathBuf,
    pub renew_before_days: u16,
}

pub struct LoadedConfig {
    pub bind: SocketAddr,
    pub admin_socket: PathBuf,
    pub server_config: quinn::ServerConfig,
    pub admission: RelayAdmission,
    pub relay: QuicRelayConfig,
}

impl Config {
    pub fn example(path: &Path) -> Self {
        let base = path.parent().unwrap_or_else(|| Path::new("."));
        Self {
            version: VERSION,
            listener: ListenerConfig {
                bind: "0.0.0.0:4433".into(),
                certificate: base.join("cert.pem"),
                private_key: base.join("key.pem"),
            },
            admin: AdminConfig {
                socket: default_admin_socket_for(path),
            },
            credentials: vec![CredentialConfig {
                id: 1,
                secret_file: base.join("credential.hex"),
            }],
            allowed_targets: vec!["203.0.113.10:443".into()],
            limits: Limits {
                max_sessions: 64,
                max_flows_per_session: 16,
                max_connections: 1_024,
                max_streams_per_connection: 64,
                connect_timeout_secs: 5,
                control_timeout_secs: 5,
                shutdown_timeout_secs: 5,
            },
            acme: None,
        }
    }
}

pub fn default_config_path() -> PathBuf {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
        .unwrap_or_else(|| PathBuf::from("."))
        .join("velum/config.toml")
}

pub fn default_admin_socket() -> PathBuf {
    std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("XDG_STATE_HOME").map(PathBuf::from))
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/state")))
        .unwrap_or_else(|| PathBuf::from("."))
        .join("velum/admin.sock")
}

pub fn default_admin_socket_for(config_path: &Path) -> PathBuf {
    if config_path == default_config_path() {
        default_admin_socket()
    } else {
        config_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(".velum-admin/admin.sock")
    }
}

pub fn read(path: &Path) -> Result<Config, String> {
    let text = fs::read_to_string(path)
        .map_err(|error| format!("cannot read configuration {}: {error}", path.display()))?;
    toml::from_str(&text)
        .map_err(|error| format!("invalid configuration {}: {error}", path.display()))
}

pub fn write(path: &Path, config: &Config) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or("configuration path has no parent directory")?;
    fs::create_dir_all(parent).map_err(|error| {
        format!(
            "cannot create configuration directory {}: {error}",
            parent.display()
        )
    })?;
    let encoded = toml::to_string_pretty(config)
        .map_err(|error| format!("cannot encode configuration: {error}"))?;
    let temporary = path.with_extension("toml.tmp");
    fs::write(&temporary, encoded).map_err(|error| {
        format!(
            "cannot write configuration {}: {error}",
            temporary.display()
        )
    })?;
    restrict_permissions(&temporary)?;
    fs::rename(&temporary, path)
        .map_err(|error| format!("cannot activate configuration {}: {error}", path.display()))
}

pub fn write_secret(path: &Path, secret: &[u8]) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or("credential path has no parent directory")?;
    fs::create_dir_all(parent).map_err(|error| {
        format!(
            "cannot create credential directory {}: {error}",
            parent.display()
        )
    })?;
    if path.exists() {
        return Err(format!("credential file {} already exists", path.display()));
    }
    let encoded = secret
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    fs::write(path, format!("{encoded}\n"))
        .map_err(|error| format!("cannot write credential file {}: {error}", path.display()))?;
    restrict_permissions(path)
}

pub fn load(path: &Path) -> Result<LoadedConfig, String> {
    let config = read(path)?;
    if config.version != VERSION {
        return Err(format!(
            "unsupported configuration version {}",
            config.version
        ));
    }
    let bind = parse_socket("listener.bind", &config.listener.bind)?;
    if config.credentials.is_empty() {
        return Err("at least one credential is required".into());
    }
    if config.allowed_targets.is_empty() {
        return Err("at least one allowed target is required".into());
    }
    let credentials = config
        .credentials
        .iter()
        .map(|entry| {
            let secret = read_secret(&entry.secret_file)?;
            PrincipalCredential::new(PrincipalId(entry.id), secret)
                .map_err(|_| format!("invalid credential {}", entry.id))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let authenticator = Authenticator::new(credentials)
        .map_err(|error| format!("invalid credential set: {error:?}"))?;
    let targets = config
        .allowed_targets
        .iter()
        .map(|target| parse_socket("allowed_targets", target))
        .collect::<Result<Vec<_>, _>>()?;
    let relay = QuicRelayConfig {
        schema_version: VERSION,
        connect_timeout: positive_duration(
            "limits.connect_timeout_secs",
            config.limits.connect_timeout_secs,
        )?,
        control_timeout: positive_duration(
            "limits.control_timeout_secs",
            config.limits.control_timeout_secs,
        )?,
        shutdown_timeout: positive_duration(
            "limits.shutdown_timeout_secs",
            config.limits.shutdown_timeout_secs,
        )?,
        max_control_bytes: 192,
        max_connections: positive_limit("limits.max_connections", config.limits.max_connections)?,
        max_streams_per_connection: positive_limit(
            "limits.max_streams_per_connection",
            config.limits.max_streams_per_connection,
        )?,
    };
    relay
        .validate()
        .map_err(|error| format!("invalid relay limits: {error:?}"))?;
    let quota = PrincipalQuota {
        max_sessions: positive_limit("limits.max_sessions", config.limits.max_sessions)?,
        max_flows_per_session: positive_limit(
            "limits.max_flows_per_session",
            config.limits.max_flows_per_session,
        )?,
    };
    let certificates = read_certificates(&config.listener.certificate)?;
    let key = read_private_key(&config.listener.private_key)?;
    let server_config = quinn::ServerConfig::with_single_cert(certificates, key)
        .map_err(|error| format!("invalid certificate or private key: {error}"))?;
    Ok(LoadedConfig {
        bind,
        admin_socket: config.admin.socket,
        server_config,
        admission: RelayAdmission {
            authenticator: Arc::new(authenticator),
            destinations: Arc::new(DestinationPolicy::new(targets)),
            quotas: Arc::new(Mutex::new(AdmissionControl::new(quota))),
        },
        relay,
    })
}

pub fn read_certificates(path: &Path) -> Result<Vec<CertificateDer<'static>>, String> {
    let file = File::open(path)
        .map_err(|error| format!("cannot read certificate {}: {error}", path.display()))?;
    let certificates = CertificateDer::pem_reader_iter(BufReader::new(file))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("cannot parse certificate {}: {error}", path.display()))?;
    if certificates.is_empty() {
        return Err(format!(
            "certificate {} contains no PEM certificates",
            path.display()
        ));
    }
    Ok(certificates)
}

pub fn read_private_key(path: &Path) -> Result<PrivateKeyDer<'static>, String> {
    verify_private_file(path)?;
    let file = File::open(path)
        .map_err(|error| format!("cannot read private key {}: {error}", path.display()))?;
    PrivateKeyDer::from_pem_reader(BufReader::new(file))
        .map_err(|error| format!("cannot parse private key {}: {error}", path.display()))
}

pub fn certificate_expiry(path: &Path) -> Result<(String, i64), String> {
    let certificate = read_certificates(path)?
        .into_iter()
        .next()
        .ok_or("certificate chain is empty")?;
    let (_, certificate) = parse_x509_certificate(certificate.as_ref())
        .map_err(|error| format!("cannot parse X.509 certificate {}: {error}", path.display()))?;
    let expires = certificate.validity().not_after;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|error| format!("system clock precedes Unix epoch: {error}"))?
        .as_secs() as i64;
    Ok((
        expires.to_string(),
        (expires.timestamp() - now).div_euclid(86_400),
    ))
}

fn verify_private_file(path: &Path) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = fs::metadata(path)
            .map_err(|error| format!("cannot inspect private key {}: {error}", path.display()))?
            .permissions()
            .mode();
        if mode & 0o077 != 0 {
            return Err(format!(
                "private key {} must not be group or world readable",
                path.display()
            ));
        }
    }
    Ok(())
}

fn read_secret(path: &Path) -> Result<Vec<u8>, String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = fs::metadata(path)
            .map_err(|error| format!("cannot inspect credential file {}: {error}", path.display()))?
            .permissions()
            .mode();
        if mode & 0o077 != 0 {
            return Err(format!(
                "credential file {} must not be group or world readable",
                path.display()
            ));
        }
    }
    let encoded = fs::read_to_string(path)
        .map_err(|error| format!("cannot read credential file {}: {error}", path.display()))?;
    decode_secret(encoded.trim())
}

fn decode_secret(value: &str) -> Result<Vec<u8>, String> {
    if value.is_empty() || !value.len().is_multiple_of(2) || value.len() / 2 > MAX_SECRET_BYTES {
        return Err("credential secret must contain 1 to 128 bytes of hexadecimal data".into());
    }
    let mut secret = Vec::with_capacity(value.len() / 2);
    for pair in value.as_bytes().chunks_exact(2) {
        let pair = std::str::from_utf8(pair).expect("hex input is UTF-8");
        secret.push(
            u8::from_str_radix(pair, 16)
                .map_err(|_| "credential secret must be hexadecimal".to_owned())?,
        );
    }
    Ok(secret)
}

fn parse_socket(name: &str, value: &str) -> Result<SocketAddr, String> {
    value
        .parse()
        .map_err(|_| format!("{name} must be an IP:PORT socket address"))
}

fn positive_limit(name: &str, value: usize) -> Result<usize, String> {
    if value == 0 {
        Err(format!("{name} must be positive"))
    } else {
        Ok(value)
    }
}

fn positive_duration(name: &str, value: u64) -> Result<Duration, String> {
    if value == 0 {
        Err(format!("{name} must be positive"))
    } else {
        Ok(Duration::from_secs(value))
    }
}

fn restrict_permissions(path: &Path) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600)).map_err(|error| {
            format!("cannot restrict permissions on {}: {error}", path.display())
        })?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn examples_round_trip_without_secrets() {
        let path = Path::new("/tmp/velum/config.toml");
        let encoded = toml::to_string_pretty(&Config::example(path)).expect("encode");
        let decoded: Config = toml::from_str(&encoded).expect("decode");
        assert_eq!(decoded.version, VERSION);
        assert_eq!(
            decoded.credentials[0].secret_file,
            Path::new("/tmp/velum/credential.hex")
        );
        assert_eq!(
            decoded.admin.socket,
            Path::new("/tmp/velum/.velum-admin/admin.sock")
        );
    }

    #[test]
    fn rejects_non_hex_secrets() {
        assert!(decode_secret("not-hex").is_err());
    }

    #[cfg(unix)]
    #[test]
    fn rejects_a_group_or_world_readable_private_key() {
        use std::os::unix::fs::PermissionsExt;

        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("velum-private-key-{unique}"));
        fs::write(&path, b"not a key").expect("key");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).expect("permissions");
        assert!(verify_private_file(&path).is_err());
        let _ = fs::remove_file(path);
    }
}

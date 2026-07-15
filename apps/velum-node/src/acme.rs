//! External ACME companion integration.
//!
//! Velum deliberately delegates ACME protocol and DNS-provider support to
//! Lego. This module owns only non-secret policy, invoking the pinned helper,
//! and activating a validated certificate/key pair with rollback on failure.

use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

use crate::config::{self, AcmeConfig, Config};

const LETS_ENCRYPT_PRODUCTION: &str = "https://acme-v02.api.letsencrypt.org/directory";
const LETS_ENCRYPT_STAGING: &str = "https://acme-staging-v02.api.letsencrypt.org/directory";
const LEGO_VERSION: &str = "5.2.2";

pub fn configure(
    config_path: &Path,
    email: String,
    domains: Vec<String>,
    dns_provider: String,
    staging: bool,
) -> Result<(), String> {
    if email.trim().is_empty() {
        return Err("ACME email must not be empty".into());
    }
    if domains.is_empty() || domains.iter().any(|domain| !valid_domain(domain)) {
        return Err(
            "supply one or more DNS names with --domain; IP addresses are unsupported".into(),
        );
    }
    if dns_provider.trim().is_empty() {
        return Err("ACME DNS provider must not be empty".into());
    }
    let mut config = config::read(config_path)?;
    config.acme = Some(AcmeConfig {
        email,
        domains,
        dns_provider,
        directory_url: if staging {
            LETS_ENCRYPT_STAGING.to_owned()
        } else {
            LETS_ENCRYPT_PRODUCTION.to_owned()
        },
        state_dir: default_state_dir(),
        renew_before_days: 30,
    });
    config::write(config_path, &config)?;
    println!("Configured DNS-01 ACME policy in {}", config_path.display());
    Ok(())
}

pub async fn obtain(config_path: &Path) -> Result<(), String> {
    run(config_path, "run").await
}

pub async fn renew(config_path: &Path) -> Result<(), String> {
    run(config_path, "renew").await
}

async fn run(config_path: &Path, action: &str) -> Result<(), String> {
    let config = config::read(config_path)?;
    let acme = config
        .acme
        .as_ref()
        .ok_or("ACME is not configured; run velum acme configure first")?;
    secure_directory(&acme.state_dir)?;
    let lego = lego_binary()?;
    let mut command = Command::new(&lego);
    command
        .arg("--accept-tos")
        .arg("--email")
        .arg(&acme.email)
        .arg("--server")
        .arg(&acme.directory_url)
        .arg("--path")
        .arg(&acme.state_dir)
        .arg("--dns")
        .arg(&acme.dns_provider);
    for domain in &acme.domains {
        command.arg("--domains").arg(domain);
    }
    command.arg(action);
    if action == "renew" {
        command
            .arg("--days")
            .arg(acme.renew_before_days.to_string());
    }
    let status = command.status().map_err(|error| {
        format!(
            "cannot execute Lego at {}: {error}; run scripts/install-lego.sh or set VELUM_LEGO_BIN",
            lego.display()
        )
    })?;
    if !status.success() {
        return Err(format!("Lego {action} failed with {status}"));
    }
    let reload_required = requires_reload(action, &config.admin.socket);
    activate(config_path, &config, acme, reload_required).await
}

fn requires_reload(action: &str, admin_socket: &Path) -> bool {
    action == "renew" || admin_socket.exists()
}

async fn activate(
    config_path: &Path,
    config: &Config,
    acme: &AcmeConfig,
    reload_required: bool,
) -> Result<(), String> {
    if config.listener.certificate == config.listener.private_key {
        return Err("ACME requires distinct certificate and private-key paths".into());
    }
    let domain = acme
        .domains
        .first()
        .ok_or("ACME requires at least one domain")?;
    let certificates = acme.state_dir.join("certificates");
    let certificate = certificates.join(format!("{domain}.crt"));
    let private_key = certificates.join(format!("{domain}.key"));
    // Validate the generated pair before replacing either live file.
    let chain = config::read_certificates(&certificate)?;
    let key = config::read_private_key(&private_key)?;
    quinn::ServerConfig::with_single_cert(chain, key)
        .map_err(|error| format!("Lego produced an invalid certificate or key: {error}"))?;
    let staged_certificate = stage_private_file(&certificate, &config.listener.certificate)?;
    let staged_private_key = match stage_private_file(&private_key, &config.listener.private_key) {
        Ok(staged) => staged,
        Err(error) => {
            cleanup_path(&staged_certificate);
            return Err(error);
        }
    };
    let activation = activate_pair(
        &staged_certificate,
        &staged_private_key,
        &config.listener.certificate,
        &config.listener.private_key,
    );
    if activation.is_err() {
        cleanup_path(&staged_certificate);
        cleanup_path(&staged_private_key);
    }
    let activation = activation?;
    println!("Activated ACME certificate for {domain}");
    if !reload_required {
        activation.commit();
        println!("The certificate will be used when the relay starts.");
        return Ok(());
    }
    match request_reload(config_path).await {
        Ok(()) => {
            activation.commit();
            Ok(())
        }
        Err(error) => activation.rollback().map_or_else(
            |rollback_error| Err(format!(
                "{error}; additionally failed to restore the previous certificate pair: {rollback_error}"
            )),
            |_| Err(format!("{error}; restored the previous certificate pair")),
        ),
    }
}

async fn request_reload(config_path: &Path) -> Result<(), String> {
    let config = config::read(config_path)?;
    crate::admin::request(&config.admin.socket, "reload").await?;
    println!("Reload completed by running service");
    Ok(())
}

fn stage_private_file(source: &Path, destination: &Path) -> Result<PathBuf, String> {
    let parent = destination
        .parent()
        .ok_or("certificate destination has no parent directory")?;
    fs::create_dir_all(parent).map_err(|error| {
        format!(
            "cannot create certificate directory {}: {error}",
            parent.display()
        )
    })?;
    let temporary = temporary_path(destination, "stage");
    if temporary.exists() {
        return Err(format!(
            "ACME staging file {} already exists; resolve the previous activation first",
            temporary.display()
        ));
    }
    fs::copy(source, &temporary).map_err(|error| {
        format!(
            "cannot stage certificate {}: {error}",
            destination.display()
        )
    })?;
    restrict_file(&temporary)?;
    Ok(temporary)
}

struct ActivatedPair {
    certificate: PathBuf,
    private_key: PathBuf,
    certificate_backup: Option<PathBuf>,
    private_key_backup: Option<PathBuf>,
}

impl ActivatedPair {
    fn commit(self) {
        cleanup(&self.certificate_backup);
        cleanup(&self.private_key_backup);
    }

    fn rollback(self) -> Result<(), String> {
        let private_key = restore_file(&self.private_key, self.private_key_backup.as_deref());
        let certificate = restore_file(&self.certificate, self.certificate_backup.as_deref());
        match (private_key, certificate) {
            (Ok(()), Ok(())) => Ok(()),
            (Err(error), Ok(())) | (Ok(()), Err(error)) => Err(error),
            (Err(private_key), Err(certificate)) => Err(format!(
                "private key rollback failed: {private_key}; certificate rollback failed: {certificate}"
            )),
        }
    }
}

fn activate_pair(
    staged_certificate: &Path,
    staged_private_key: &Path,
    certificate: &Path,
    private_key: &Path,
) -> Result<ActivatedPair, String> {
    let certificate_backup = backup_file(certificate)?;
    let private_key_backup = match backup_file(private_key) {
        Ok(backup) => backup,
        Err(error) => {
            cleanup(&certificate_backup);
            return Err(error);
        }
    };

    if let Err(error) = replace_file(staged_certificate, certificate) {
        cleanup(&certificate_backup);
        cleanup(&private_key_backup);
        return Err(error);
    }
    if let Err(error) = replace_file(staged_private_key, private_key) {
        let rollback = restore_file(certificate, certificate_backup.as_deref());
        cleanup(&private_key_backup);
        return match rollback {
            Ok(()) => Err(error),
            Err(rollback_error) => Err(format!(
                "{error}; additionally failed to restore certificate {}: {rollback_error}",
                certificate.display()
            )),
        };
    }

    Ok(ActivatedPair {
        certificate: certificate.to_owned(),
        private_key: private_key.to_owned(),
        certificate_backup,
        private_key_backup,
    })
}

fn backup_file(path: &Path) -> Result<Option<PathBuf>, String> {
    if !path.exists() {
        return Ok(None);
    }
    let backup = temporary_path(path, "backup");
    if backup.exists() {
        return Err(format!(
            "ACME backup file {} already exists; resolve the previous activation first",
            backup.display()
        ));
    }
    fs::copy(path, &backup)
        .map_err(|error| format!("cannot back up certificate {}: {error}", path.display()))?;
    restrict_file(&backup)?;
    Ok(Some(backup))
}

fn replace_file(staged: &Path, destination: &Path) -> Result<(), String> {
    fs::rename(staged, destination).map_err(|error| {
        format!(
            "cannot activate certificate {}: {error}",
            destination.display()
        )
    })
}

fn restore_file(destination: &Path, backup: Option<&Path>) -> Result<(), String> {
    match backup {
        Some(backup) => fs::rename(backup, destination).map_err(|error| {
            format!(
                "cannot restore certificate {} from {}: {error}",
                destination.display(),
                backup.display()
            )
        }),
        None if destination.exists() => fs::remove_file(destination).map_err(|error| {
            format!(
                "cannot remove certificate {}: {error}",
                destination.display()
            )
        }),
        None => Ok(()),
    }
}

fn cleanup(path: &Option<PathBuf>) {
    if let Some(path) = path {
        cleanup_path(path);
    }
}

fn cleanup_path(path: &Path) {
    let _ = fs::remove_file(path);
}

fn temporary_path(destination: &Path, label: &str) -> PathBuf {
    destination.with_extension(format!("velum-acme-{label}"))
}

fn lego_binary() -> Result<PathBuf, String> {
    if let Some(path) = env::var_os("VELUM_LEGO_BIN") {
        return Ok(PathBuf::from(path));
    }
    let data = env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/share")))
        .ok_or("cannot determine Velum data directory; set VELUM_LEGO_BIN")?;
    Ok(data
        .join("velum/tools/lego")
        .join(format!("v{LEGO_VERSION}"))
        .join("lego"))
}

fn default_state_dir() -> PathBuf {
    env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/state")))
        .unwrap_or_else(|| PathBuf::from("."))
        .join("velum/acme")
}

fn valid_domain(domain: &str) -> bool {
    !domain.is_empty()
        && domain.len() <= 253
        && domain.bytes().any(|byte| byte.is_ascii_alphabetic())
        && domain.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && label
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        })
}

fn secure_directory(path: &Path) -> Result<(), String> {
    fs::create_dir_all(path).map_err(|error| {
        format!(
            "cannot create ACME state directory {}: {error}",
            path.display()
        )
    })?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700)).map_err(|error| {
            format!(
                "cannot secure ACME state directory {}: {error}",
                path.display()
            )
        })?;
    }
    Ok(())
}

fn restrict_file(path: &Path) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600)).map_err(|error| {
            format!("cannot secure certificate file {}: {error}", path.display())
        })?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_dns_names_and_rejects_ip_addresses() {
        assert!(valid_domain("relay.example.com"));
        assert!(!valid_domain("127.0.0.1"));
    }

    #[test]
    fn initial_obtain_does_not_require_a_running_service() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let socket = std::env::temp_dir().join(format!("velum-no-admin-{unique}.sock"));

        assert!(!requires_reload("run", &socket));
        assert!(requires_reload("renew", &socket));
    }

    #[test]
    fn pair_activation_replaces_both_staged_files() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let directory = std::env::temp_dir().join(format!("velum-acme-{unique}"));
        fs::create_dir_all(&directory).expect("directory");
        let certificate = directory.join("cert.pem");
        let private_key = directory.join("key.pem");
        let staged_certificate = temporary_path(&certificate, "stage");
        let staged_private_key = temporary_path(&private_key, "stage");
        fs::write(&certificate, b"old certificate").expect("certificate");
        fs::write(&private_key, b"old key").expect("key");
        fs::write(&staged_certificate, b"new certificate").expect("staged certificate");
        fs::write(&staged_private_key, b"new key").expect("staged key");

        activate_pair(
            &staged_certificate,
            &staged_private_key,
            &certificate,
            &private_key,
        )
        .expect("activate pair")
        .commit();

        assert_eq!(
            fs::read(&certificate).expect("certificate"),
            b"new certificate"
        );
        assert_eq!(fs::read(&private_key).expect("key"), b"new key");
        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    fn pair_activation_restores_the_certificate_when_key_replacement_fails() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let directory = std::env::temp_dir().join(format!("velum-acme-{unique}"));
        fs::create_dir_all(&directory).expect("directory");
        let certificate = directory.join("cert.pem");
        let private_key = directory.join("key.pem");
        let staged_certificate = temporary_path(&certificate, "stage");
        let missing_staged_key = temporary_path(&private_key, "stage");
        fs::write(&certificate, b"old certificate").expect("certificate");
        fs::write(&private_key, b"old key").expect("key");
        fs::write(&staged_certificate, b"new certificate").expect("staged certificate");

        assert!(
            activate_pair(
                &staged_certificate,
                &missing_staged_key,
                &certificate,
                &private_key,
            )
            .is_err()
        );

        assert_eq!(
            fs::read(&certificate).expect("certificate"),
            b"old certificate"
        );
        assert_eq!(fs::read(&private_key).expect("key"), b"old key");
        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    fn pair_activation_can_restore_both_files_after_a_reload_failure() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let directory = std::env::temp_dir().join(format!("velum-acme-{unique}"));
        fs::create_dir_all(&directory).expect("directory");
        let certificate = directory.join("cert.pem");
        let private_key = directory.join("key.pem");
        let staged_certificate = temporary_path(&certificate, "stage");
        let staged_private_key = temporary_path(&private_key, "stage");
        fs::write(&certificate, b"old certificate").expect("certificate");
        fs::write(&private_key, b"old key").expect("key");
        fs::write(&staged_certificate, b"new certificate").expect("staged certificate");
        fs::write(&staged_private_key, b"new key").expect("staged key");

        activate_pair(
            &staged_certificate,
            &staged_private_key,
            &certificate,
            &private_key,
        )
        .expect("activate pair")
        .rollback()
        .expect("restore pair");

        assert_eq!(
            fs::read(&certificate).expect("certificate"),
            b"old certificate"
        );
        assert_eq!(fs::read(&private_key).expect("key"), b"old key");
        let _ = fs::remove_dir_all(directory);
    }
}

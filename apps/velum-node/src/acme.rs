//! External ACME companion integration.
//!
//! Velum deliberately delegates ACME protocol and DNS-provider support to
//! Lego. This module owns only non-secret policy, invoking the pinned helper,
//! and atomically activating the resulting PEM files.

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
    activate(config_path, &config, acme).await
}

async fn activate(config_path: &Path, config: &Config, acme: &AcmeConfig) -> Result<(), String> {
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
    install_private_file(&certificate, &config.listener.certificate)?;
    install_private_file(&private_key, &config.listener.private_key)?;
    println!("Activated ACME certificate for {domain}");
    request_reload(config_path).await
}

async fn request_reload(config_path: &Path) -> Result<(), String> {
    let config = config::read(config_path)?;
    crate::admin::request(&config.admin.socket, "reload").await?;
    println!("Reload requested from running service");
    Ok(())
}

fn install_private_file(source: &Path, destination: &Path) -> Result<(), String> {
    if source == destination {
        return Ok(());
    }
    let parent = destination
        .parent()
        .ok_or("certificate destination has no parent directory")?;
    fs::create_dir_all(parent).map_err(|error| {
        format!(
            "cannot create certificate directory {}: {error}",
            parent.display()
        )
    })?;
    let temporary = destination.with_extension("velum-acme.tmp");
    fs::copy(source, &temporary).map_err(|error| {
        format!(
            "cannot stage certificate {}: {error}",
            destination.display()
        )
    })?;
    restrict_file(&temporary)?;
    fs::rename(&temporary, destination).map_err(|error| {
        format!(
            "cannot activate certificate {}: {error}",
            destination.display()
        )
    })
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
}

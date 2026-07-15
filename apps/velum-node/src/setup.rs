//! Resumable interactive relay and certificate provisioning.

use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::{
    acme,
    config::{self, Config, CoverServiceConfig},
    enrollment,
    terminal::{
        prompt_choice, prompt_default, prompt_positive_u64, prompt_positive_usize, prompt_required,
        prompt_yes_no,
    },
};

pub async fn run(path: PathBuf) -> Result<(), String> {
    let (mut config, existed) = load_or_example(&path)?;
    if !existed {
        config.listener.bind = config::random_listener_bind()?;
    } else {
        println!("Resuming or reconfiguring {}", path.display());
    }
    config.listener.bind = prompt_default("UDP listener address", &config.listener.bind)?;
    if config.allowed_targets.is_empty() {
        config.allowed_targets.push("203.0.113.10:443".into());
    }
    config.allowed_targets[0] = prompt_default("Allowed TCP target", &config.allowed_targets[0])?;
    configure_cover_service(&mut config)?;

    let secret_path = config.credentials[0].secret_file.clone();
    let created_secret = ensure_credential(&secret_path)?;
    config::write(&path, &config)?;
    println!(
        "{} configuration at {}",
        if existed { "Updated" } else { "Created" },
        path.display()
    );
    if created_secret {
        println!("Created a 32-byte credential at {}", secret_path.display());
        println!("Give the credential file only to the authorized client configuration.");
    }

    println!("\nCertificate source");
    println!("1. Request a CA certificate with ACME DNS-01");
    println!("2. Use an existing certificate and private key");
    println!("3. Generate a self-signed certificate");
    match prompt_choice(
        "Choose a certificate source",
        &["ACME", "existing", "self-signed"],
    )? {
        0 => configure_acme_certificate(&path, &config).await?,
        1 => configure_existing_certificate(&path, &mut config)?,
        2 => configure_self_signed_certificate(&path, &mut config)?,
        _ => unreachable!("prompt_choice validates the selection"),
    }
    println!("Guided setup completed successfully.");
    if prompt_yes_no("Create a client enrollment now", true)? {
        let detected_public_ip = crate::public_ip::detect().await;
        match detected_public_ip {
            Some(address) => println!("Detected public relay IP from IPinfo: {address}"),
            None => println!(
                "Could not detect a public relay IP. Enter the client-reachable IP address manually."
            ),
        }
        enrollment::interactive_issue_with_suggested_relay(&path, detected_public_ip)?;
    }
    println!("Next: validate the configuration, then start or deploy the relay.");
    Ok(())
}

async fn configure_acme_certificate(path: &Path, config: &Config) -> Result<(), String> {
    let existing_acme = config.acme.as_ref();
    let email = prompt_required(
        "ACME account email",
        existing_acme.map(|configured| configured.email.as_str()),
    )?;
    let domain = prompt_required(
        "Relay DNS name",
        existing_acme.and_then(|configured| configured.domains.first().map(String::as_str)),
    )?;
    let dns_provider = prompt_required(
        "Lego DNS provider",
        existing_acme.map(|configured| configured.dns_provider.as_str()),
    )?;
    let staging = prompt_yes_no(
        "Use the Let's Encrypt staging service",
        existing_acme.is_some_and(|configured| configured.directory_url.contains("staging")),
    )?;

    acme::configure(path, email, vec![domain], dns_provider, staging)?;
    println!("Requesting the certificate with the DNS credentials in this environment...");
    acme::obtain(path).await
}

fn configure_existing_certificate(path: &Path, config: &mut Config) -> Result<(), String> {
    config.listener.certificate = PathBuf::from(prompt_required(
        "Existing PEM certificate path",
        Some(&config.listener.certificate.display().to_string()),
    )?);
    config.listener.private_key = PathBuf::from(prompt_required(
        "Existing PEM private key path",
        Some(&config.listener.private_key.display().to_string()),
    )?);
    config.acme = None;
    config::write(path, config)?;
    config::load(path).map(|_| ())
}

fn configure_self_signed_certificate(path: &Path, config: &mut Config) -> Result<(), String> {
    let server_name = prompt_required("Certificate DNS name or IP address", Some("localhost"))?;
    let rcgen::CertifiedKey { cert, signing_key } =
        rcgen::generate_simple_self_signed(vec![server_name.clone()])
            .map_err(|error| format!("cannot generate self-signed certificate: {error}"))?;
    write_new_certificate_pair(
        &config.listener.certificate,
        cert.pem().as_bytes(),
        &config.listener.private_key,
        signing_key.serialize_pem().as_bytes(),
    )?;
    config.acme = None;
    config::write(path, config)?;
    config::load(path).map(|_| ())?;
    println!("Generated a self-signed certificate for {server_name}.");
    println!(
        "Trust {} explicitly in each authorized client.",
        config.listener.certificate.display()
    );
    Ok(())
}

fn load_or_example(path: &Path) -> Result<(Config, bool), String> {
    if path.exists() {
        config::read(path).map(|config| (config, true))
    } else {
        Ok((Config::example(path), false))
    }
}

fn ensure_credential(path: &Path) -> Result<bool, String> {
    if path.is_file() {
        return Ok(false);
    }
    let mut secret = [0_u8; 32];
    getrandom::fill(&mut secret).map_err(|error| format!("cannot generate credential: {error}"))?;
    config::write_secret(path, &secret)?;
    Ok(true)
}

fn write_new_certificate_pair(
    certificate: &Path,
    certificate_pem: &[u8],
    private_key: &Path,
    private_key_pem: &[u8],
) -> Result<(), String> {
    if certificate == private_key {
        return Err("certificate and private-key paths must be different".into());
    }
    if certificate.exists() || private_key.exists() {
        return Err(format!(
            "certificate destination already exists ({} or {}); choose the existing-certificate option or remove both files explicitly",
            certificate.display(),
            private_key.display()
        ));
    }
    let certificate_stage = certificate.with_extension("pem.setup-stage");
    let private_key_stage = private_key.with_extension("pem.setup-stage");
    write_staged_private_file(&certificate_stage, certificate_pem)?;
    if let Err(error) = write_staged_private_file(&private_key_stage, private_key_pem) {
        let _ = fs::remove_file(&certificate_stage);
        return Err(error);
    }
    if let Err(error) = fs::rename(&certificate_stage, certificate) {
        let _ = fs::remove_file(&certificate_stage);
        let _ = fs::remove_file(&private_key_stage);
        return Err(format!(
            "cannot activate certificate {}: {error}",
            certificate.display()
        ));
    }
    if let Err(error) = fs::rename(&private_key_stage, private_key) {
        let _ = fs::remove_file(certificate);
        let _ = fs::remove_file(&private_key_stage);
        return Err(format!(
            "cannot activate private key {}: {error}",
            private_key.display()
        ));
    }
    Ok(())
}

fn write_staged_private_file(path: &Path, contents: &[u8]) -> Result<(), String> {
    if path.exists() {
        return Err(format!(
            "setup staging file {} already exists; remove it after checking the previous setup attempt",
            path.display()
        ));
    }
    let parent = path
        .parent()
        .ok_or("certificate path has no parent directory")?;
    fs::create_dir_all(parent).map_err(|error| {
        format!(
            "cannot create certificate directory {}: {error}",
            parent.display()
        )
    })?;
    fs::write(path, contents).map_err(|error| {
        format!(
            "cannot write certificate material {}: {error}",
            path.display()
        )
    })?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600)).map_err(|error| {
            format!(
                "cannot secure certificate material {}: {error}",
                path.display()
            )
        })?;
    }
    Ok(())
}

fn configure_cover_service(config: &mut Config) -> Result<(), String> {
    let enabled = prompt_yes_no(
        "Enable optional reverse-proxy cover service",
        config.cover_service.is_some(),
    )?;
    if !enabled {
        config.cover_service = None;
        return Ok(());
    }

    let existing = config.cover_service.as_ref();
    config.cover_service = Some(CoverServiceConfig {
        bind: prompt_default(
            "Cover TCP listener address",
            existing
                .map(|cover| cover.bind.as_str())
                .unwrap_or(&config.listener.bind),
        )?,
        upstream: prompt_default(
            "Cover reverse-proxy upstream",
            existing
                .map(|cover| cover.upstream.as_str())
                .unwrap_or("127.0.0.1:8080"),
        )?,
        request_head_timeout_secs: prompt_positive_u64(
            "Cover request-head timeout seconds",
            existing.map_or(5, |cover| cover.request_head_timeout_secs),
        )?,
        upstream_timeout_secs: prompt_positive_u64(
            "Cover upstream timeout seconds",
            existing.map_or(5, |cover| cover.upstream_timeout_secs),
        )?,
        max_connections: prompt_positive_usize(
            "Cover maximum connections",
            existing.map_or(256, |cover| cover.max_connections),
        )?,
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    fn temporary_directory(label: &str) -> PathBuf {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        std::env::temp_dir().join(format!("velum-setup-{label}-{unique}"))
    }

    #[test]
    fn existing_configuration_is_loaded_for_resume() {
        let directory = temporary_directory("resume");
        let path = directory.join("config.toml");
        let mut expected = Config::example(&path);
        expected.listener.bind = "127.0.0.1:8443".into();
        config::write(&path, &expected).expect("write configuration");

        let (loaded, existed) = load_or_example(&path).expect("resume configuration");

        assert!(existed);
        assert_eq!(loaded.listener.bind, "127.0.0.1:8443");
        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    fn existing_credential_is_never_replaced() {
        let directory = temporary_directory("credential");
        let path = directory.join("credential.hex");
        fs::create_dir_all(&directory).expect("directory");
        fs::write(&path, "operator-owned\n").expect("credential");

        assert!(!ensure_credential(&path).expect("reuse credential"));
        assert_eq!(
            fs::read_to_string(&path).expect("credential"),
            "operator-owned\n"
        );
        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    fn self_signed_pair_is_written_without_overwriting_existing_material() {
        let directory = temporary_directory("self-signed");
        let certificate = directory.join("cert.pem");
        let private_key = directory.join("key.pem");

        write_new_certificate_pair(&certificate, b"certificate", &private_key, b"key")
            .expect("write pair");
        assert_eq!(fs::read(&certificate).expect("certificate"), b"certificate");
        assert!(write_new_certificate_pair(&certificate, b"new", &private_key, b"new").is_err());
        let _ = fs::remove_dir_all(directory);
    }
}

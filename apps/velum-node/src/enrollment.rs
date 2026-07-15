//! Client credential issuance, revocation, and offline enrollment delivery.

use std::{
    fs::{self, OpenOptions},
    io::Write,
    net::SocketAddr,
    path::{Path, PathBuf},
};

use qrcode::QrCode;
use velum_client_profile::{EnrollmentBundle, EnrollmentNode, EnrollmentTrust};

use crate::{
    config::{self, CredentialConfig},
    terminal::{prompt_choice, prompt_default, prompt_required},
};

enum Delivery {
    Qr,
    File(PathBuf),
}

struct IssueOptions {
    config_path: PathBuf,
    client_name: String,
    relay_address: SocketAddr,
    server_name: String,
    custom_ca: bool,
    delivery: Delivery,
}

enum Selector {
    Id(u64),
    Name(String),
}

pub fn command(action: &str, arguments: &[String]) -> Result<(), String> {
    match action {
        "issue" => issue(parse_issue(arguments)?),
        "revoke" => revoke(parse_revoke(arguments)?),
        _ => Err("client command must be issue or revoke".into()),
    }
}

pub fn interactive_issue(config_path: &Path) -> Result<(), String> {
    let config = config::read(config_path)?;
    let client_name = prompt_required("Client device name", None)?;
    let suggested_relay = suggested_relay(&config.listener.bind);
    let relay = prompt_required("Client-reachable relay IP:PORT", suggested_relay.as_deref())?;
    let relay_address = parse_relay(&relay)?;
    let suggested_name = config
        .acme
        .as_ref()
        .and_then(|acme| acme.domains.first())
        .map(String::as_str);
    let server_name = prompt_required("TLS server name", suggested_name)?;
    println!("Trust mode");
    println!("1. System trust store");
    println!("2. Include configured certificate as a custom CA");
    let custom_ca = prompt_choice("Choose trust mode", &["system", "custom CA"])? == 1;
    println!("Enrollment delivery");
    println!("1. Show a QR code for a mobile client");
    println!("2. Write an owner-only .velum-enroll file");
    let delivery = match prompt_choice("Choose delivery", &["QR", "file"])? {
        0 => Delivery::Qr,
        1 => {
            let default = config_path
                .parent()
                .unwrap_or_else(|| Path::new("."))
                .join(format!("{}.velum-enroll", slug(&client_name)));
            let path = prompt_default("Enrollment file", &default.display().to_string())?;
            Delivery::File(PathBuf::from(path))
        }
        _ => unreachable!("prompt_choice bounds the result"),
    };
    issue(IssueOptions {
        config_path: config_path.to_path_buf(),
        client_name,
        relay_address,
        server_name,
        custom_ca,
        delivery,
    })
}

pub fn interactive_revoke(config_path: &Path) -> Result<(), String> {
    let config = config::read(config_path)?;
    let issued = config
        .credentials
        .iter()
        .filter_map(|credential| {
            credential
                .name
                .as_ref()
                .map(|name| format!("{}: {}", credential.id, name))
        })
        .collect::<Vec<_>>();
    if issued.is_empty() {
        return Err("no named client credentials are configured".into());
    }
    println!("Issued clients:");
    for client in issued {
        println!("  {client}");
    }
    let name = prompt_required("Client device name to revoke", None)?;
    revoke((config_path.to_path_buf(), Selector::Name(name)))
}

fn parse_issue(arguments: &[String]) -> Result<IssueOptions, String> {
    let mut config_path = config::default_config_path();
    let mut client_name = None;
    let mut relay_address = None;
    let mut server_name = None;
    let mut custom_ca = false;
    let mut delivery = None;
    let mut index = 0;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--config" => config_path = PathBuf::from(next(arguments, &mut index, "--config")?),
            "--name" => client_name = Some(next(arguments, &mut index, "--name")?),
            "--relay" => {
                relay_address = Some(parse_relay(&next(arguments, &mut index, "--relay")?)?)
            }
            "--server-name" => server_name = Some(next(arguments, &mut index, "--server-name")?),
            "--trust" => match next(arguments, &mut index, "--trust")?.as_str() {
                "system" => custom_ca = false,
                "custom-ca" => custom_ca = true,
                value => {
                    return Err(format!(
                        "unknown trust mode {value}; expected system or custom-ca"
                    ));
                }
            },
            "--qr" if delivery.is_none() => delivery = Some(Delivery::Qr),
            "--output" if delivery.is_none() => {
                delivery = Some(Delivery::File(PathBuf::from(next(
                    arguments, &mut index, "--output",
                )?)))
            }
            "--qr" | "--output" => return Err("choose exactly one of --qr or --output PATH".into()),
            flag => return Err(format!("unknown client issue option {flag}")),
        }
        index += 1;
    }
    Ok(IssueOptions {
        config_path,
        client_name: client_name.ok_or("client issue requires --name NAME")?,
        relay_address: relay_address.ok_or("client issue requires --relay IP:PORT")?,
        server_name: server_name.ok_or("client issue requires --server-name NAME")?,
        custom_ca,
        delivery: delivery.ok_or("client issue requires --qr or --output PATH")?,
    })
}

fn parse_revoke(arguments: &[String]) -> Result<(PathBuf, Selector), String> {
    let mut config_path = config::default_config_path();
    let mut selector = None;
    let mut index = 0;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--config" => config_path = PathBuf::from(next(arguments, &mut index, "--config")?),
            "--id" if selector.is_none() => {
                let value = next(arguments, &mut index, "--id")?;
                selector = Some(Selector::Id(
                    value.parse().map_err(|_| "--id must be an integer")?,
                ));
            }
            "--name" if selector.is_none() => {
                selector = Some(Selector::Name(next(arguments, &mut index, "--name")?))
            }
            "--id" | "--name" => return Err("choose exactly one of --id or --name".into()),
            flag => return Err(format!("unknown client revoke option {flag}")),
        }
        index += 1;
    }
    Ok((
        config_path,
        selector.ok_or("client revoke requires --id ID or --name NAME")?,
    ))
}

fn issue(options: IssueOptions) -> Result<(), String> {
    validate_client_name(&options.client_name)?;
    let original = config::read(&options.config_path)?;
    for credential in &original.credentials {
        if config::read_secret(&credential.secret_file)?.len() != 32 {
            return Err(format!(
                "credential {} is not 32 bytes; rotate legacy credentials before issuing an enrollment",
                credential.id
            ));
        }
    }
    if original
        .credentials
        .iter()
        .any(|credential| credential.name.as_deref() == Some(&options.client_name))
    {
        return Err(format!(
            "client {} is already configured",
            options.client_name
        ));
    }
    let principal_id = original
        .credentials
        .iter()
        .map(|credential| credential.id)
        .max()
        .unwrap_or(0)
        .checked_add(1)
        .ok_or("principal id space is exhausted")?;
    let config_parent = options
        .config_path
        .parent()
        .ok_or("configuration path has no parent directory")?;
    let secret_file = config_parent
        .join("credentials")
        .join(format!("{}-{principal_id}.hex", slug(&options.client_name)));
    let mut secret = [0_u8; 32];
    getrandom::fill(&mut secret).map_err(|error| format!("cannot generate credential: {error}"))?;
    let trust = if options.custom_ca {
        EnrollmentTrust::CustomCa {
            certificate_pem: fs::read_to_string(&original.listener.certificate).map_err(
                |error| {
                    format!(
                        "cannot read certificate {}: {error}",
                        original.listener.certificate.display()
                    )
                },
            )?,
        }
    } else {
        EnrollmentTrust::System
    };
    let bundle = EnrollmentBundle::new(
        EnrollmentNode {
            id: format!("relay-{principal_id}"),
            name: options.server_name.clone(),
            relay_address: options.relay_address,
            server_name: options.server_name,
        },
        principal_id,
        &secret,
        trust,
    )
    .map_err(|error| format!("invalid enrollment: {error}"))?;
    let canonical = bundle
        .to_canonical_json()
        .map_err(|error| format!("cannot encode enrollment: {error}"))?;
    let qr = match options.delivery {
        Delivery::Qr => Some(render_qr(&canonical)?),
        Delivery::File(_) => None,
    };

    config::write_secret(&secret_file, &secret)?;
    secret.fill(0);
    let mut updated = original.clone();
    updated.credentials.push(CredentialConfig {
        id: principal_id,
        name: Some(options.client_name.clone()),
        secret_file: secret_file.clone(),
    });
    if let Err(error) = config::write(&options.config_path, &updated) {
        let _ = fs::remove_file(&secret_file);
        return Err(error);
    }

    let delivery_result = match options.delivery {
        Delivery::Qr => {
            println!("\nScan this enrollment only with the authorized client:\n");
            println!(
                "{}",
                qr.expect("QR was rendered before configuration changed")
            );
            Ok(())
        }
        Delivery::File(path) => write_private_new(&path, canonical.as_bytes()).map(|()| {
            println!("Created owner-only enrollment file at {}", path.display());
        }),
    };
    if let Err(error) = delivery_result {
        let rollback = config::write(&options.config_path, &original);
        let _ = fs::remove_file(&secret_file);
        return match rollback {
            Ok(()) => Err(error),
            Err(rollback) => Err(format!(
                "{error}; configuration rollback also failed: {rollback}"
            )),
        };
    }
    println!(
        "Issued principal {principal_id} for {}.",
        options.client_name
    );
    println!("The enrollment contains a long-lived secret; import it once and remove every copy.");
    println!("Restart or redeploy a running relay before this credential becomes active.");
    Ok(())
}

fn revoke((config_path, selector): (PathBuf, Selector)) -> Result<(), String> {
    let mut config = config::read(&config_path)?;
    if config.credentials.len() == 1 {
        return Err("cannot revoke the final configured credential".into());
    }
    let position = config
        .credentials
        .iter()
        .position(|credential| match &selector {
            Selector::Id(id) => credential.id == *id,
            Selector::Name(name) => credential.name.as_ref() == Some(name),
        })
        .ok_or("client credential was not found")?;
    let removed = config.credentials.remove(position);
    config::write(&config_path, &config)?;
    match fs::remove_file(&removed.secret_file) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(format!(
                "principal {} was revoked, but credential cleanup failed at {}: {error}",
                removed.id,
                removed.secret_file.display()
            ));
        }
    }
    println!("Revoked principal {}.", removed.id);
    println!("Restart or redeploy a running relay to complete revocation.");
    Ok(())
}

fn render_qr(payload: &str) -> Result<String, String> {
    QrCode::new(payload.as_bytes())
        .map(|code| {
            code.render::<char>()
                .quiet_zone(true)
                .module_dimensions(2, 1)
                .dark_color('█')
                .light_color(' ')
                .build()
        })
        .map_err(|_| "enrollment is too large for a QR code; use --output instead".into())
}

fn write_private_new(path: &Path, contents: &[u8]) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or("enrollment path has no parent directory")?;
    fs::create_dir_all(parent).map_err(|error| {
        format!(
            "cannot create enrollment directory {}: {error}",
            parent.display()
        )
    })?;
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(path)
        .map_err(|error| format!("cannot create enrollment file {}: {error}", path.display()))?;
    if let Err(error) = file
        .write_all(contents)
        .and_then(|()| file.write_all(b"\n"))
    {
        let _ = fs::remove_file(path);
        return Err(format!(
            "cannot write enrollment file {}: {error}",
            path.display()
        ));
    }
    Ok(())
}

fn validate_client_name(name: &str) -> Result<(), String> {
    if name.trim().is_empty() || name.len() > 64 || name.chars().any(char::is_control) {
        Err("client name must contain 1 to 64 printable characters".into())
    } else {
        Ok(())
    }
}

fn slug(value: &str) -> String {
    let slug = value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>();
    let slug = slug.trim_matches('-');
    if slug.is_empty() {
        "client".into()
    } else {
        slug.into()
    }
}

fn parse_relay(value: &str) -> Result<SocketAddr, String> {
    let address = value
        .parse::<SocketAddr>()
        .map_err(|_| "relay must be an IP:PORT socket address")?;
    if address.ip().is_unspecified() {
        return Err("relay must be client reachable, not an unspecified address".into());
    }
    Ok(address)
}

fn suggested_relay(bind: &str) -> Option<String> {
    let address = bind.parse::<SocketAddr>().ok()?;
    (!address.ip().is_unspecified()).then(|| address.to_string())
}

fn next(arguments: &[String], index: &mut usize, flag: &str) -> Result<String, String> {
    *index += 1;
    arguments
        .get(*index)
        .cloned()
        .ok_or_else(|| format!("{flag} requires a value"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temporary_directory(label: &str) -> PathBuf {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("velum-enrollment-{label}-{unique}"));
        fs::create_dir_all(&path).expect("temporary directory");
        path
    }

    #[test]
    fn rejects_unspecified_relay_and_ambiguous_delivery() {
        assert!(parse_relay("0.0.0.0:4433").is_err());
        let arguments = [
            "--name",
            "phone",
            "--relay",
            "127.0.0.1:4433",
            "--server-name",
            "localhost",
            "--qr",
            "--output",
            "phone.velum-enroll",
        ]
        .map(str::to_owned);
        assert!(parse_issue(&arguments).is_err());
    }

    #[test]
    fn qr_renderer_accepts_a_normal_enrollment_payload() {
        assert!(render_qr("{\"kind\":\"velum-enrollment\"}").is_ok());
    }

    #[test]
    fn generated_file_names_do_not_accept_path_segments() {
        assert_eq!(slug("Alice's / Phone"), "alice-s---phone");
        assert!(validate_client_name("phone").is_ok());
        assert!(validate_client_name("").is_err());
        assert_eq!(slug("移动端"), "client");
    }

    #[test]
    fn file_enrollment_issues_matching_credential_and_can_revoke_it() {
        let directory = temporary_directory("lifecycle");
        let config_path = directory.join("config.toml");
        let output = directory.join("phone.velum-enroll");
        let configuration = crate::config::Config::example(&config_path);
        config::write_secret(&configuration.credentials[0].secret_file, &[3; 32])
            .expect("initial credential");
        config::write(&config_path, &configuration).expect("configuration");

        issue(IssueOptions {
            config_path: config_path.clone(),
            client_name: "phone".into(),
            relay_address: "203.0.113.10:4433".parse().expect("relay"),
            server_name: "relay.example".into(),
            custom_ca: false,
            delivery: Delivery::File(output.clone()),
        })
        .expect("issue");

        let configured = config::read(&config_path).expect("updated configuration");
        let credential = configured
            .credentials
            .iter()
            .find(|credential| credential.name.as_deref() == Some("phone"))
            .expect("issued credential");
        let server_secret = fs::read_to_string(&credential.secret_file).expect("server secret");
        let bundle = EnrollmentBundle::from_json(
            fs::read_to_string(&output)
                .expect("enrollment")
                .trim()
                .as_bytes(),
        )
        .expect("valid enrollment");
        assert_eq!(bundle.credential, server_secret.trim());

        let secret_path = credential.secret_file.clone();
        revoke((config_path.clone(), Selector::Name("phone".into()))).expect("revoke");
        assert!(!secret_path.exists());
        assert_eq!(
            config::read(&config_path)
                .expect("configuration")
                .credentials
                .len(),
            1
        );

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(&output)
                    .expect("enrollment metadata")
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );
        }
        fs::remove_dir_all(directory).expect("cleanup");
    }
}

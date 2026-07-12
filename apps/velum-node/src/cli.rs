//! Operator-facing command line and guided terminal console.

use std::{
    io::{self, IsTerminal, Write},
    path::PathBuf,
    sync::Arc,
};

use crate::{
    NoopRelayObserver, acme, admin, bind_quic_listener,
    config::{self, Config},
    deployment, serve_quic_listener,
};

const USAGE: &str = "\
Usage:
  velum                         Open the guided operator console
  velum init [--config PATH]    Create a versioned configuration template
  velum setup [--config PATH]   Interactively create a configuration and credential
  velum serve [--config PATH]   Start the experimental QUIC relay
  velum deploy [--config PATH] [--binary PATH] [--dry-run]
                                Validate, install, and start a systemd user service
  velum config validate [--config PATH]
                                Validate configuration, credentials, and TLS material
  velum cert verify [--config PATH]
                                Verify configured certificate and private key load
  velum acme configure --email EMAIL --dns PROVIDER --domain NAME [--domain NAME]
                       [--staging] [--config PATH]
                                Configure external Lego DNS-01 issuance
  velum acme obtain|renew [--config PATH]
                                Issue or renew, install, and reload certificates
  velum status [--format text|json] [--config PATH]
  velum drain|shutdown [--config PATH]
                                Control a running local service
  velum help
  velum --version
";

pub async fn run(arguments: impl IntoIterator<Item = String>) -> Result<(), String> {
    let arguments = arguments.into_iter().collect::<Vec<_>>();
    match arguments.as_slice() {
        [] => interactive().await,
        [command] if command == "help" || command == "--help" => {
            print!("{USAGE}");
            Ok(())
        }
        [command] if command == "--version" || command == "version" => {
            println!(
                "velum {} ({})",
                env!("CARGO_PKG_VERSION"),
                option_env!("VELUM_BUILD_REVISION").unwrap_or("dev")
            );
            Ok(())
        }
        arguments if arguments.iter().any(|argument| argument == "--help") => {
            print!("{USAGE}");
            Ok(())
        }
        [command, rest @ ..] if command == "init" => initialize(config_path(rest)?),
        [command, rest @ ..] if command == "setup" => setup(config_path(rest)?),
        [command, argument] if command == "serve" && argument == "--help" => {
            print!("{USAGE}");
            Ok(())
        }
        [command, rest @ ..] if command == "serve" => serve(config_path(rest)?).await,
        [command, rest @ ..] if command == "deploy" => deploy(rest),
        [command, action, rest @ ..] if command == "config" && action == "validate" => {
            validate(config_path(rest)?)
        }
        [command, action, rest @ ..] if command == "cert" && action == "verify" => {
            verify_certificate(config_path(rest)?)
        }
        [command, action, rest @ ..] if command == "acme" && action == "configure" => {
            let configured = acme_configuration(rest)?;
            acme::configure(
                &configured.config_path,
                configured.email,
                configured.domains,
                configured.dns_provider,
                configured.staging,
            )
        }
        [command, action, rest @ ..] if command == "acme" && action == "obtain" => {
            acme::obtain(&config_path(rest)?).await
        }
        [command, action, rest @ ..] if command == "acme" && action == "renew" => {
            acme::renew(&config_path(rest)?).await
        }
        [command, rest @ ..] if matches!(command.as_str(), "status" | "drain" | "shutdown") => {
            control(command, control_options(rest)?).await
        }
        _ => Err(format!("unknown command\n\n{USAGE}")),
    }
}

fn deploy(arguments: &[String]) -> Result<(), String> {
    let mut config_path = config::default_config_path();
    let mut binary = None;
    let mut dry_run = false;
    let mut index = 0;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--config" => {
                config_path = PathBuf::from(next_value(arguments, &mut index, "--config")?)
            }
            "--binary" => {
                binary = Some(PathBuf::from(next_value(
                    arguments, &mut index, "--binary",
                )?))
            }
            "--dry-run" => dry_run = true,
            flag => return Err(format!("unknown deploy option {flag}")),
        }
        index += 1;
    }
    deployment::deploy(&config_path, binary, dry_run)
}

struct AcmeConfiguration {
    config_path: PathBuf,
    email: String,
    domains: Vec<String>,
    dns_provider: String,
    staging: bool,
}

fn acme_configuration(arguments: &[String]) -> Result<AcmeConfiguration, String> {
    let mut config_path = config::default_config_path();
    let mut email = None;
    let mut domains = Vec::new();
    let mut dns_provider = None;
    let mut staging = false;
    let mut index = 0;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--email" => {
                email = Some(next_value(arguments, &mut index, "--email")?);
            }
            "--domain" => domains.push(next_value(arguments, &mut index, "--domain")?),
            "--dns" => dns_provider = Some(next_value(arguments, &mut index, "--dns")?),
            "--config" => {
                config_path = PathBuf::from(next_value(arguments, &mut index, "--config")?)
            }
            "--staging" => staging = true,
            flag => return Err(format!("unknown ACME option {flag}")),
        }
        index += 1;
    }
    Ok(AcmeConfiguration {
        config_path,
        email: email.ok_or("ACME requires --email EMAIL")?,
        domains,
        dns_provider: dns_provider.ok_or("ACME requires --dns PROVIDER")?,
        staging,
    })
}

fn next_value(arguments: &[String], index: &mut usize, flag: &str) -> Result<String, String> {
    *index += 1;
    arguments
        .get(*index)
        .cloned()
        .ok_or_else(|| format!("{flag} requires a value"))
}

fn config_path(arguments: &[String]) -> Result<PathBuf, String> {
    match arguments {
        [] => Ok(config::default_config_path()),
        [flag, path] if flag == "--config" => Ok(PathBuf::from(path)),
        _ => Err("expected optional --config PATH".into()),
    }
}

fn initialize(path: PathBuf) -> Result<(), String> {
    if path.exists() {
        return Err(format!("configuration {} already exists", path.display()));
    }
    config::write(&path, &Config::example(&path))?;
    println!("Created configuration template at {}", path.display());
    println!(
        "Set the certificate, private key, allowed target, and credential secret file before serving."
    );
    Ok(())
}

fn setup(path: PathBuf) -> Result<(), String> {
    if path.exists() {
        return Err(format!("configuration {} already exists", path.display()));
    }
    let mut config = Config::example(&path);
    config.listener.bind = prompt_default("UDP listener address", &config.listener.bind)?;
    config.listener.certificate = PathBuf::from(prompt_default(
        "PEM certificate path",
        &config.listener.certificate.display().to_string(),
    )?);
    config.listener.private_key = PathBuf::from(prompt_default(
        "PEM private key path",
        &config.listener.private_key.display().to_string(),
    )?);
    config.allowed_targets[0] = prompt_default("Allowed TCP target", &config.allowed_targets[0])?;
    let secret_path = config.credentials[0].secret_file.clone();
    let mut secret = [0_u8; 32];
    getrandom::fill(&mut secret).map_err(|error| format!("cannot generate credential: {error}"))?;
    config::write_secret(&secret_path, &secret)?;
    if let Err(error) = config::write(&path, &config) {
        let _ = std::fs::remove_file(&secret_path);
        return Err(error);
    }
    println!("Created configuration at {}", path.display());
    println!("Created a 32-byte credential at {}", secret_path.display());
    println!("Give the credential file only to the authorized client configuration.");
    Ok(())
}

fn validate(path: PathBuf) -> Result<(), String> {
    let loaded = config::load(&path)?;
    println!(
        "Configuration valid: listener {}, admin socket {}",
        loaded.bind,
        loaded.admin_socket.display()
    );
    Ok(())
}

fn verify_certificate(path: PathBuf) -> Result<(), String> {
    let loaded = config::load(&path)?;
    let configured = config::read(&path)?;
    let (expires_at, days_remaining) =
        config::certificate_expiry(&configured.listener.certificate)?;
    if days_remaining < 0 {
        return Err(format!("certificate expired at {expires_at}"));
    }
    println!(
        "Certificate and private key are valid for listener {}\nexpires_at={expires_at}\ndays_remaining={days_remaining}",
        loaded.bind,
    );
    Ok(())
}

async fn serve(path: PathBuf) -> Result<(), String> {
    let loaded = config::load(&path)?;
    let endpoint = bind_quic_listener(loaded.bind, loaded.server_config)
        .map_err(|error| format!("cannot bind {}: {error}", loaded.bind))?;
    let address = endpoint
        .local_addr()
        .map_err(|error| format!("cannot determine listener address: {error}"))?;
    eprintln!("velum listening on {address}");
    let (controls, control_requests) = tokio::sync::mpsc::channel(4);
    let signal_controls = controls.clone();
    let status = Arc::new(admin::RuntimeStatus::default());
    let admin = admin::spawn(
        loaded.admin_socket.clone(),
        address.to_string(),
        controls,
        Arc::clone(&status),
    )?;
    let signal_task = tokio::spawn(async move {
        wait_for_shutdown().await;
        let _ = signal_controls.send(admin::Control::Shutdown).await;
    });
    let result = serve_quic_listener(
        endpoint,
        loaded.admission,
        loaded.relay,
        Arc::new(NoopRelayObserver),
        status,
        control_requests,
        || {
            let reloaded = config::load(&path)?;
            if reloaded.bind != address {
                return Err("listener.bind cannot change during reload".into());
            }
            Ok(reloaded.server_config)
        },
    )
    .await;
    signal_task.abort();
    admin.stop();
    result
}

struct ControlOptions {
    config_path: PathBuf,
    json: bool,
}

fn control_options(arguments: &[String]) -> Result<ControlOptions, String> {
    let mut config_path = config::default_config_path();
    let mut json = false;
    let mut index = 0;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--config" => {
                config_path = PathBuf::from(next_value(arguments, &mut index, "--config")?)
            }
            "--format" => match next_value(arguments, &mut index, "--format")?.as_str() {
                "text" => json = false,
                "json" => json = true,
                value => {
                    return Err(format!(
                        "unknown status format {value}; expected text or json"
                    ));
                }
            },
            flag => return Err(format!("unknown control option {flag}")),
        }
        index += 1;
    }
    Ok(ControlOptions { config_path, json })
}

async fn control(command: &str, options: ControlOptions) -> Result<(), String> {
    if options.json && command != "status" {
        return Err("--format is supported only by status".into());
    }
    let config = config::read(&options.config_path)?;
    let request = if options.json { "status json" } else { command };
    let response = admin::request(&config.admin.socket, request).await?;
    print!("{response}");
    Ok(())
}

async fn interactive() -> Result<(), String> {
    if !atty() {
        return Err(format!(
            "no command was supplied outside an interactive terminal\n\n{USAGE}"
        ));
    }
    let path = config::default_config_path();
    loop {
        println!("\nVelum operator console");
        println!("Configuration: {}", path.display());
        println!("1. Guided first-time setup");
        println!("2. Validate configuration and credentials");
        println!("3. Verify certificate and private key");
        println!("4. Start relay");
        println!("5. Show service status");
        println!("6. Drain service");
        println!("7. Shut down service");
        println!("8. Exit");
        match prompt("Choose an action")?.as_str() {
            "1" => report(setup(path.clone())),
            "2" => report(validate(path.clone())),
            "3" => report(verify_certificate(path.clone())),
            "4" => return serve(path).await,
            "5" => report(
                control(
                    "status",
                    ControlOptions {
                        config_path: path.clone(),
                        json: false,
                    },
                )
                .await,
            ),
            "6" => report(
                control(
                    "drain",
                    ControlOptions {
                        config_path: path.clone(),
                        json: false,
                    },
                )
                .await,
            ),
            "7" => report(
                control(
                    "shutdown",
                    ControlOptions {
                        config_path: path.clone(),
                        json: false,
                    },
                )
                .await,
            ),
            "8" | "q" | "quit" => return Ok(()),
            _ => println!("Choose a number from 1 to 8."),
        }
    }
}

fn prompt(label: &str) -> Result<String, String> {
    print!("{label}: ");
    io::stdout()
        .flush()
        .map_err(|error| format!("cannot write prompt: {error}"))?;
    let mut value = String::new();
    io::stdin()
        .read_line(&mut value)
        .map_err(|error| format!("cannot read input: {error}"))?;
    Ok(value.trim().to_owned())
}

fn prompt_default(label: &str, default: &str) -> Result<String, String> {
    let value = prompt(&format!("{label} [{default}]"))?;
    Ok(if value.is_empty() {
        default.to_owned()
    } else {
        value
    })
}

fn report(result: Result<(), String>) {
    match result {
        Ok(()) => {}
        Err(error) => println!("Action failed: {error}"),
    }
}

fn atty() -> bool {
    io::stdin().is_terminal()
}

#[cfg(unix)]
async fn wait_for_shutdown() {
    use tokio::signal::unix::{SignalKind, signal};

    let mut terminate = signal(SignalKind::terminate()).expect("install SIGTERM handler");
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {},
        _ = terminate.recv() => {},
    }
}

#[cfg(not(unix))]
async fn wait_for_shutdown() {
    tokio::signal::ctrl_c()
        .await
        .expect("install Ctrl-C handler");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_only_the_optional_config_path() {
        assert_eq!(
            config_path(&[]).expect("default"),
            config::default_config_path()
        );
        assert_eq!(
            config_path(&["--config".into(), "example.toml".into()]).expect("custom"),
            PathBuf::from("example.toml")
        );
        assert!(config_path(&["--bad".into()]).is_err());
    }

    #[test]
    fn parses_acme_configuration_in_any_option_order() {
        let parsed = acme_configuration(&[
            "--domain".into(),
            "relay.example.com".into(),
            "--staging".into(),
            "--dns".into(),
            "cloudflare".into(),
            "--config".into(),
            "relay.toml".into(),
            "--email".into(),
            "ops@example.com".into(),
        ])
        .expect("configuration");
        assert_eq!(parsed.config_path, PathBuf::from("relay.toml"));
        assert_eq!(parsed.domains, ["relay.example.com"]);
        assert!(parsed.staging);
    }

    #[test]
    fn rejects_unknown_deploy_options() {
        assert!(deploy(&["--bad".into()]).is_err());
    }
}

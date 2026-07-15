//! Operator-facing command line and guided terminal console.

use std::{
    io::{self, IsTerminal},
    path::{Path, PathBuf},
    sync::Arc,
};

use crate::{
    NoopRelayObserver, acme, admin, bind_cover_listener, bind_quic_listener,
    config::{self, Config},
    deployment, enrollment, serve_cover_listener, serve_quic_listener, setup,
    terminal::prompt,
};

const USAGE: &str = "\
Usage:
  velum                         Open the guided operator console
  velum init [--config PATH]    Create a versioned configuration template
  velum setup [--config PATH]   Interactively create a configuration and credential
  velum serve [--config PATH]   Start the experimental QUIC relay and optional cover service
  velum deploy [--config PATH] [--binary PATH] [--dry-run]
                                Validate, install, and start a systemd user service
  velum uninstall [--config PATH] [--purge] [--yes]
                                Stop and remove a systemd user service
  velum config validate [--config PATH]
                                Validate configuration, credentials, and TLS material
  velum cert verify [--config PATH]
                                Verify configured certificate and private key load
  velum client issue --name NAME --relay IP:PORT --server-name NAME
                     (--qr | --output PATH) [--trust system|custom-ca] [--config PATH]
                                Issue one device credential and enrollment
  velum client revoke (--id ID | --name NAME) [--config PATH]
                                Revoke one issued device credential
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
        [command, rest @ ..] if command == "setup" => setup::run(config_path(rest)?).await,
        [command, argument] if command == "serve" && argument == "--help" => {
            print!("{USAGE}");
            Ok(())
        }
        [command, rest @ ..] if command == "serve" => serve(config_path(rest)?).await,
        [command, rest @ ..] if command == "deploy" => deploy(rest),
        [command, rest @ ..] if command == "uninstall" => uninstall(rest),
        [command, action, rest @ ..] if command == "config" && action == "validate" => {
            validate(config_path(rest)?)
        }
        [command, action, rest @ ..] if command == "cert" && action == "verify" => {
            verify_certificate(config_path(rest)?)
        }
        [command, action, rest @ ..] if command == "client" => enrollment::command(action, rest),
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

fn uninstall(arguments: &[String]) -> Result<(), String> {
    let mut config_path = config::default_config_path();
    let mut purge = false;
    let mut confirmed = false;
    let mut index = 0;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--config" => {
                config_path = PathBuf::from(next_value(arguments, &mut index, "--config")?)
            }
            "--purge" => purge = true,
            "--yes" => confirmed = true,
            flag => return Err(format!("unknown uninstall option {flag}")),
        }
        index += 1;
    }

    if !confirmed {
        if !atty() {
            return Err("uninstall requires --yes outside an interactive terminal".into());
        }
        println!(
            "This stops and removes the deployment for {}.",
            config_path.display()
        );
        if purge {
            println!("The configuration file will also be removed.");
        } else {
            println!(
                "The configuration, credentials, certificates, binary, and Lego tool will be preserved."
            );
        }
        confirmed = prompt("Type yes to continue")? == "yes";
    }
    if !confirmed {
        println!("Uninstall cancelled.");
        return Ok(());
    }

    let report = crate::uninstall::run(&config_path, purge)?;
    if report.service_removed {
        println!("Removed the systemd user service.");
    } else {
        println!("No deployed systemd user service was found.");
    }
    if report.configuration_removed {
        println!("Removed the configuration file.");
    }
    println!("Credentials, certificates, the Velum binary, and the Lego tool were preserved.");
    Ok(())
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
    let mut configuration = Config::example(&path);
    configuration.listener.bind = config::random_listener_bind()?;
    config::write(&path, &configuration)?;
    println!("Created configuration template at {}", path.display());
    println!(
        "Set the certificate, private key, allowed target, and credential secret file before serving."
    );
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
    let cover_listener = match loaded.cover_service.as_ref() {
        Some(cover) => Some(
            bind_cover_listener(cover.bind)
                .await
                .map_err(|error| format!("cannot bind cover service: {error}"))?,
        ),
        None => None,
    };
    let endpoint = bind_quic_listener(loaded.bind, loaded.server_config)
        .map_err(|error| format!("cannot bind {}: {error}", loaded.bind))?;
    let address = endpoint
        .local_addr()
        .map_err(|error| format!("cannot determine listener address: {error}"))?;
    eprintln!("velum listening on {address}");
    let (cover_shutdown, cover_shutdown_request) = tokio::sync::watch::channel(false);
    let cover_task = match (cover_listener, loaded.cover_service.clone()) {
        (Some(listener), Some(cover)) => {
            let address = listener
                .local_addr()
                .map_err(|error| format!("cannot determine cover listener address: {error}"))?;
            eprintln!("velum cover service listening on {address}");
            Some(tokio::spawn(serve_cover_listener(
                listener,
                cover.config,
                cover.max_connections,
                cover_shutdown_request,
            )))
        }
        (None, None) => None,
        _ => return Err("cover listener configuration changed during startup".into()),
    };
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
    let _ = cover_shutdown.send(true);
    if let Some(cover_task) = cover_task {
        let _ = cover_task.await;
    }
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
        println!("1. Guided setup or reconfigure");
        println!("2. Obtain or replace ACME certificate");
        println!("3. Renew ACME certificate");
        println!("4. Validate configuration and credentials");
        println!("5. Verify certificate and private key");
        println!("6. Issue a client enrollment");
        println!("7. Revoke a client credential");
        println!("8. Start relay");
        println!("9. Show service status");
        println!("10. Drain service");
        println!("11. Shut down service");
        println!("12. Uninstall this deployment");
        println!("13. Exit");
        match prompt("Choose an action")?.as_str() {
            "1" => report(setup::run(path.clone()).await),
            "2" => report(acme::obtain(&path).await),
            "3" => report(acme::renew(&path).await),
            "4" => report(validate(path.clone())),
            "5" => report(verify_certificate(path.clone())),
            "6" => report(enrollment::interactive_issue(&path)),
            "7" => report(enrollment::interactive_revoke(&path)),
            "8" => return serve(path).await,
            "9" => report(
                control(
                    "status",
                    ControlOptions {
                        config_path: path.clone(),
                        json: false,
                    },
                )
                .await,
            ),
            "10" => report(
                control(
                    "drain",
                    ControlOptions {
                        config_path: path.clone(),
                        json: false,
                    },
                )
                .await,
            ),
            "11" => report(
                control(
                    "shutdown",
                    ControlOptions {
                        config_path: path.clone(),
                        json: false,
                    },
                )
                .await,
            ),
            "12" => report(interactive_uninstall(&path)),
            "13" | "q" | "quit" => return Ok(()),
            _ => println!("Choose a number from 1 to 13."),
        }
    }
}

fn interactive_uninstall(path: &Path) -> Result<(), String> {
    println!(
        "This stops and removes the deployment for {}.",
        path.display()
    );
    let purge = crate::terminal::prompt_yes_no("Remove the configuration file too", false)?;
    if !crate::terminal::prompt_yes_no("Continue with uninstall", false)? {
        println!("Uninstall cancelled.");
        return Ok(());
    }
    let report = crate::uninstall::run(path, purge)?;
    if report.service_removed {
        println!("Removed the systemd user service.");
    } else {
        println!("No deployed systemd user service was found.");
    }
    if report.configuration_removed {
        println!("Removed the configuration file.");
    }
    println!("Credentials, certificates, the Velum binary, and the Lego tool were preserved.");
    Ok(())
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

    #[test]
    fn uninstall_requires_explicit_confirmation_outside_a_terminal() {
        assert!(uninstall(&[]).is_err());
        assert!(uninstall(&["--bad".into()]).is_err());
    }
}

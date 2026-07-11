//! Operator-facing command line and guided terminal console.

use std::{
    io::{self, IsTerminal, Write},
    path::PathBuf,
    sync::Arc,
};

use crate::{
    NoopRelayObserver, admin, bind_quic_listener,
    config::{self, Config},
    serve_quic_listener,
};

const USAGE: &str = "\
Usage:
  velum                         Open the guided operator console
  velum init [--config PATH]    Create a versioned configuration template
  velum setup [--config PATH]   Interactively create a configuration and credential
  velum serve [--config PATH]   Start the experimental QUIC relay
  velum config validate [--config PATH]
                                Validate configuration, credentials, and TLS material
  velum cert verify [--config PATH]
                                Verify configured certificate and private key load
  velum status|drain|shutdown [--config PATH]
                                Control a running local service
  velum help
";

pub async fn run(arguments: impl IntoIterator<Item = String>) -> Result<(), String> {
    let arguments = arguments.into_iter().collect::<Vec<_>>();
    match arguments.as_slice() {
        [] => interactive().await,
        [command] if command == "help" || command == "--help" => {
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
        [command, action, rest @ ..] if command == "config" && action == "validate" => {
            validate(config_path(rest)?)
        }
        [command, action, rest @ ..] if command == "cert" && action == "verify" => {
            verify_certificate(config_path(rest)?)
        }
        [command, rest @ ..] if matches!(command.as_str(), "status" | "drain" | "shutdown") => {
            control(command, config_path(rest)?).await
        }
        _ => Err(format!("unknown command\n\n{USAGE}")),
    }
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
    config::write(&path, &config)?;
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
    println!(
        "Certificate and private key are valid for listener {}",
        loaded.bind
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
    let (controls, mut control_requests) = tokio::sync::mpsc::channel(4);
    let admin = admin::spawn(loaded.admin_socket.clone(), address.to_string(), controls)?;
    serve_quic_listener(
        endpoint,
        loaded.admission,
        loaded.relay,
        Arc::new(NoopRelayObserver),
        async move {
            tokio::select! {
                _ = wait_for_shutdown() => {},
                _ = control_requests.recv() => {},
            }
        },
    )
    .await;
    admin.stop();
    Ok(())
}

async fn control(command: &str, path: PathBuf) -> Result<(), String> {
    let config = config::read(&path)?;
    let response = admin::request(&config.admin.socket, command).await?;
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
            "5" => report(control("status", path.clone()).await),
            "6" => report(control("drain", path.clone()).await),
            "7" => report(control("shutdown", path.clone()).await),
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
}

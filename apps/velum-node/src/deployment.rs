//! Systemd user-service deployment for a validated local configuration.

use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

use crate::config;

pub fn deploy(config_path: &Path, binary: Option<PathBuf>, dry_run: bool) -> Result<(), String> {
    let configured = config_path.canonicalize().map_err(|error| {
        format!(
            "cannot resolve configuration {}: {error}",
            config_path.display()
        )
    })?;
    let config = config::read(&configured)?;
    // Loading also validates credentials, TLS material, and all relay limits before
    // a unit is written or an existing service is restarted.
    config::load(&configured)?;
    let binary = binary
        .unwrap_or_else(|| env::current_exe().unwrap_or_else(|_| PathBuf::from("velum")))
        .canonicalize()
        .map_err(|error| format!("cannot resolve velum binary: {error}"))?;
    validate_unit_path(&configured, "configuration")?;
    validate_unit_path(&binary, "velum binary")?;
    let unit_name = unit_name(&configured);
    let unit_path = user_unit_directory()?.join(&unit_name);
    let unit = render_unit(&binary, &configured, config.limits.shutdown_timeout_secs);

    if dry_run {
        print!("# {}\n{unit}", unit_path.display());
        return Ok(());
    }

    write_private_file(&unit_path, &unit)?;
    let active = systemctl(&["is-active", "--quiet", &unit_name]).is_ok();
    systemctl(&["daemon-reload"])?;
    systemctl(&["enable", &unit_name])?;
    if active {
        systemctl(&["restart", &unit_name])?;
    } else {
        systemctl(&["start", &unit_name])?;
    }
    println!("Deployed and started {unit_name}");
    Ok(())
}

/// Stop and remove the user service associated with one configuration.
pub fn undeploy(config_path: &Path) -> Result<bool, String> {
    let configured = config_path.canonicalize().map_err(|error| {
        format!(
            "cannot resolve configuration {}: {error}",
            config_path.display()
        )
    })?;
    let unit_name = unit_name(&configured);
    let unit_path = user_unit_directory()?.join(&unit_name);
    if !unit_path.exists() {
        return Ok(false);
    }

    systemctl(&["disable", "--now", &unit_name])?;
    fs::remove_file(&unit_path).map_err(|error| {
        format!(
            "cannot remove systemd unit {}: {error}",
            unit_path.display()
        )
    })?;
    systemctl(&["daemon-reload"])?;
    Ok(true)
}

fn user_unit_directory() -> Result<PathBuf, String> {
    let base = env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
        .ok_or("cannot determine user configuration directory")?;
    Ok(base.join("systemd/user"))
}

fn unit_name(config: &Path) -> String {
    // FNV-1a makes separate, stable unit names for independent configurations.
    let hash = config
        .as_os_str()
        .as_encoded_bytes()
        .iter()
        .fold(0xcbf29ce484222325_u64, |hash, byte| {
            (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
        });
    format!("velum-{hash:016x}.service")
}

fn render_unit(binary: &Path, config: &Path, shutdown_timeout_secs: u64) -> String {
    format!(
        "[Unit]\nDescription=Velum experimental relay\nAfter=network-online.target\nWants=network-online.target\n\n[Service]\nType=simple\nExecStart={} serve --config {}\nRestart=on-failure\nRestartSec=5\nKillSignal=SIGTERM\nTimeoutStopSec={}\nUMask=0077\nNoNewPrivileges=true\nPrivateTmp=true\n\n[Install]\nWantedBy=default.target\n",
        systemd_argument(binary),
        systemd_argument(config),
        shutdown_timeout_secs.saturating_add(5),
    )
}

fn systemd_argument(value: &Path) -> String {
    let value = value.to_string_lossy();
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

fn validate_unit_path(path: &Path, label: &str) -> Result<(), String> {
    let value = path
        .to_str()
        .ok_or_else(|| format!("{label} path must be valid UTF-8 for systemd"))?;
    if value.chars().any(char::is_control) {
        return Err(format!("{label} path must not contain control characters"));
    }
    Ok(())
}

fn write_private_file(path: &Path, contents: &str) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or("systemd unit path has no parent directory")?;
    fs::create_dir_all(parent).map_err(|error| {
        format!(
            "cannot create systemd user directory {}: {error}",
            parent.display()
        )
    })?;
    let temporary = path.with_extension("service.tmp");
    fs::write(&temporary, contents)
        .map_err(|error| format!("cannot write systemd unit {}: {error}", temporary.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&temporary, fs::Permissions::from_mode(0o600)).map_err(|error| {
            format!(
                "cannot secure systemd unit {}: {error}",
                temporary.display()
            )
        })?;
    }
    fs::rename(&temporary, path)
        .map_err(|error| format!("cannot activate systemd unit {}: {error}", path.display()))
}

fn systemctl(arguments: &[&str]) -> Result<(), String> {
    let status = Command::new("systemctl")
        .arg("--user")
        .args(arguments)
        .status()
        .map_err(|error| format!("cannot execute systemctl --user: {error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "systemctl --user {} failed with {status}",
            arguments.join(" ")
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unit_name_is_stable_and_configuration_scoped() {
        assert_eq!(
            unit_name(Path::new("/srv/a/config.toml")),
            unit_name(Path::new("/srv/a/config.toml"))
        );
        assert_ne!(
            unit_name(Path::new("/srv/a/config.toml")),
            unit_name(Path::new("/srv/b/config.toml"))
        );
    }

    #[test]
    fn unit_quotes_paths_and_has_restart_policy() {
        let unit = render_unit(
            Path::new("/opt/Velum Relay/velum"),
            Path::new("/srv/relay/config.toml"),
            10,
        );
        assert!(unit.contains(
            "ExecStart=\"/opt/Velum Relay/velum\" serve --config \"/srv/relay/config.toml\""
        ));
        assert!(unit.contains("Restart=on-failure"));
        assert!(unit.contains("TimeoutStopSec=15"));
    }

    #[test]
    fn rejects_paths_that_cannot_be_represented_safely_in_a_unit() {
        assert!(validate_unit_path(Path::new("/srv/relay\nunit"), "configuration").is_err());
    }
}

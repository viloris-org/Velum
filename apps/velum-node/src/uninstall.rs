//! Removal of one locally deployed relay without guessing ownership of secrets.

use std::{fs, path::Path};

use crate::{config, deployment};

pub struct Report {
    pub service_removed: bool,
    pub configuration_removed: bool,
}

pub fn run(config_path: &Path, purge: bool) -> Result<Report, String> {
    let configured = config_path.canonicalize().map_err(|error| {
        format!(
            "cannot resolve configuration {}: {error}",
            config_path.display()
        )
    })?;
    let configuration = config::read(&configured)?;
    let service_removed = deployment::undeploy(&configured)?;
    remove_if_present(&configuration.admin.socket, "administration socket")?;

    let configuration_removed = if purge {
        remove_if_present(&configured, "configuration file")?;
        true
    } else {
        false
    };

    Ok(Report {
        service_removed,
        configuration_removed,
    })
}

fn remove_if_present(path: &Path, label: &str) -> Result<(), String> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!("cannot remove {label} {}: {error}", path.display())),
    }
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf};

    use super::remove_if_present;

    fn temporary_path(label: &str) -> PathBuf {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        std::env::temp_dir().join(format!("velum-uninstall-{label}-{unique}"))
    }

    #[test]
    fn removes_existing_managed_file() {
        let path = temporary_path("remove");
        fs::write(&path, "managed").expect("fixture");

        remove_if_present(&path, "fixture").expect("remove");

        assert!(!path.exists());
    }

    #[test]
    fn ignores_absent_managed_file() {
        remove_if_present(&temporary_path("absent"), "fixture").expect("absent is safe");
    }
}

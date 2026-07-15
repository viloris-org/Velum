//! Transactional lifecycle shared by privileged desktop traffic hosts.
//!
//! Authentication and native handle transfer belong to the platform IPC
//! transport. An authorized request reaches this state machine only after peer
//! identity checks. The state machine journals before mutation, rejects older
//! profile generations, and makes retries idempotent.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use velum_helper_protocol::{
    Capability, Command, CommandResult, ErrorCode, HostState, PROTOCOL_VERSION, Request, Response,
    StartParameters, Success,
};

const MAX_CACHED_RESPONSES: usize = 256;

/// Root-owned recovery record written before any route or DNS mutation.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RecoveryRecord {
    pub profile_generation: u64,
    pub configuration: StartParameters,
}

pub trait RecoveryJournal {
    fn load(&self) -> Result<Option<RecoveryRecord>, HostFailure>;
    fn write(&self, record: &RecoveryRecord) -> Result<(), HostFailure>;
    fn clear(&self) -> Result<(), HostFailure>;
}

/// Platform-specific privileged operations. No method accepts executable paths.
pub trait PlatformBackend {
    fn start(&mut self, configuration: &StartParameters) -> Result<(), HostFailure>;
    fn stop(&mut self) -> Result<(), HostFailure>;
    fn recover(&mut self, record: &RecoveryRecord) -> Result<(), HostFailure>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HostFailure {
    InvalidConfiguration,
    Platform,
    Journal,
}

/// Authorized helper request handler with deterministic recovery semantics.
pub struct TrafficHost<B, J> {
    backend: B,
    journal: J,
    capabilities: BTreeSet<Capability>,
    state: HostState,
    generation: u64,
    recovery_required: bool,
    responses: BTreeMap<u64, Response>,
}

impl<B: PlatformBackend, J: RecoveryJournal> TrafficHost<B, J> {
    pub fn new(
        backend: B,
        journal: J,
        capabilities: BTreeSet<Capability>,
    ) -> Result<Self, HostFailure> {
        let recovery_required = journal.load()?.is_some();
        Ok(Self {
            backend,
            journal,
            capabilities,
            state: if recovery_required {
                HostState::Recovering
            } else {
                HostState::Stopped
            },
            generation: 0,
            recovery_required,
            responses: BTreeMap::new(),
        })
    }

    pub const fn state(&self) -> HostState {
        self.state
    }

    pub fn handle(&mut self, request: Request) -> Response {
        if let Some(response) = self.responses.get(&request.request_id) {
            return response.clone();
        }
        let result = if request.version != PROTOCOL_VERSION {
            CommandResult::Err(ErrorCode::UnsupportedVersion)
        } else if !request.capabilities.is_subset(&self.capabilities) {
            CommandResult::Err(ErrorCode::UnsupportedCapability)
        } else if request.profile_generation < self.generation {
            CommandResult::Err(ErrorCode::GenerationConflict)
        } else {
            self.execute(&request)
        };
        let response = Response {
            version: PROTOCOL_VERSION,
            request_id: request.request_id,
            profile_generation: request.profile_generation,
            capabilities: self.capabilities.clone(),
            response: result,
        };
        self.cache(response.clone());
        response
    }

    fn execute(&mut self, request: &Request) -> CommandResult {
        match &request.command {
            Command::Hello(_) => CommandResult::Ok(Success::Hello),
            Command::Status(_) => CommandResult::Ok(Success::Status { state: self.state }),
            Command::Recover(_) => self.recover(),
            Command::Start(configuration) => {
                if self.recovery_required {
                    return CommandResult::Err(ErrorCode::RecoveryRequired);
                }
                if !valid_configuration(configuration) {
                    return CommandResult::Err(ErrorCode::InvalidConfiguration);
                }
                if matches!(self.state, HostState::Starting | HostState::Stopping) {
                    return CommandResult::Err(ErrorCode::Busy);
                }
                if self.state == HostState::Running && request.profile_generation == self.generation
                {
                    return CommandResult::Ok(Success::Started);
                }
                let record = RecoveryRecord {
                    profile_generation: request.profile_generation,
                    configuration: configuration.clone(),
                };
                if self.journal.write(&record).is_err() {
                    self.state = HostState::Failed;
                    return CommandResult::Err(ErrorCode::Platform);
                }
                self.state = HostState::Starting;
                if self.backend.start(configuration).is_err() {
                    self.state = HostState::Recovering;
                    self.recovery_required = true;
                    return CommandResult::Err(ErrorCode::Platform);
                }
                self.generation = request.profile_generation;
                self.state = HostState::Running;
                CommandResult::Ok(Success::Started)
            }
            Command::Stop(_) => {
                if self.recovery_required {
                    return CommandResult::Err(ErrorCode::RecoveryRequired);
                }
                if self.state == HostState::Stopped {
                    return CommandResult::Ok(Success::Stopped);
                }
                self.state = HostState::Stopping;
                if self.backend.stop().is_err() || self.journal.clear().is_err() {
                    self.state = HostState::Recovering;
                    self.recovery_required = true;
                    return CommandResult::Err(ErrorCode::Platform);
                }
                self.generation = self.generation.max(request.profile_generation);
                self.state = HostState::Stopped;
                CommandResult::Ok(Success::Stopped)
            }
        }
    }

    fn recover(&mut self) -> CommandResult {
        self.state = HostState::Recovering;
        let record = match self.journal.load() {
            Ok(Some(record)) => record,
            Ok(None) => {
                self.recovery_required = false;
                self.state = HostState::Stopped;
                return CommandResult::Ok(Success::Recovered);
            }
            Err(_) => return CommandResult::Err(ErrorCode::Platform),
        };
        if self.backend.recover(&record).is_err() || self.journal.clear().is_err() {
            self.recovery_required = true;
            return CommandResult::Err(ErrorCode::Platform);
        }
        self.generation = self.generation.max(record.profile_generation);
        self.recovery_required = false;
        self.state = HostState::Stopped;
        CommandResult::Ok(Success::Recovered)
    }

    fn cache(&mut self, response: Response) {
        self.responses.insert(response.request_id, response);
        while self.responses.len() > MAX_CACHED_RESPONSES {
            let Some(oldest) = self.responses.keys().next().copied() else {
                break;
            };
            self.responses.remove(&oldest);
        }
    }
}

fn valid_configuration(configuration: &StartParameters) -> bool {
    if !(576..=9000).contains(&configuration.mtu)
        || configuration.ipv4.is_none() && configuration.ipv6.is_none()
        || configuration.dns.is_empty()
        || configuration.dns.len() > 16
    {
        return false;
    }
    [configuration.ipv4, configuration.ipv6]
        .into_iter()
        .flatten()
        .all(|network| match network.address {
            std::net::IpAddr::V4(_) => network.prefix_length <= 32,
            std::net::IpAddr::V6(_) => network.prefix_length <= 128,
        })
}

/// Atomic file journal used by service and extension wrappers.
pub struct FileRecoveryJournal {
    path: PathBuf,
}

impl FileRecoveryJournal {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    fn temporary_path(&self) -> PathBuf {
        let mut value = self.path.as_os_str().to_owned();
        value.push(".tmp");
        value.into()
    }

    fn reject_symlink(path: &Path) -> Result<(), HostFailure> {
        match fs::symlink_metadata(path) {
            Ok(metadata) if metadata.file_type().is_symlink() => Err(HostFailure::Journal),
            Ok(_) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(_) => Err(HostFailure::Journal),
        }
    }
}

impl RecoveryJournal for FileRecoveryJournal {
    fn load(&self) -> Result<Option<RecoveryRecord>, HostFailure> {
        Self::reject_symlink(&self.path)?;
        let source = match fs::read(&self.path) {
            Ok(source) => source,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(_) => return Err(HostFailure::Journal),
        };
        serde_json::from_slice(&source)
            .map(Some)
            .map_err(|_| HostFailure::Journal)
    }

    fn write(&self, record: &RecoveryRecord) -> Result<(), HostFailure> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(|_| HostFailure::Journal)?;
        }
        Self::reject_symlink(&self.path)?;
        let temporary = self.temporary_path();
        Self::reject_symlink(&temporary)?;
        let encoded = serde_json::to_vec(record).map_err(|_| HostFailure::Journal)?;
        let mut file = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&temporary)
            .map_err(|_| HostFailure::Journal)?;
        file.write_all(&encoded).map_err(|_| HostFailure::Journal)?;
        file.sync_all().map_err(|_| HostFailure::Journal)?;
        fs::rename(&temporary, &self.path).map_err(|_| HostFailure::Journal)?;
        if let Some(parent) = self.path.parent() {
            File::open(parent)
                .and_then(|directory| directory.sync_all())
                .map_err(|_| HostFailure::Journal)?;
        }
        Ok(())
    }

    fn clear(&self) -> Result<(), HostFailure> {
        Self::reject_symlink(&self.path)?;
        match fs::remove_file(&self.path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(_) => Err(HostFailure::Journal),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use velum_helper_protocol::{EmptyParameters, IpNetwork};

    use super::*;

    #[derive(Default)]
    struct MemoryJournal(RefCell<Option<RecoveryRecord>>);

    impl RecoveryJournal for MemoryJournal {
        fn load(&self) -> Result<Option<RecoveryRecord>, HostFailure> {
            Ok(self.0.borrow().clone())
        }

        fn write(&self, record: &RecoveryRecord) -> Result<(), HostFailure> {
            self.0.replace(Some(record.clone()));
            Ok(())
        }

        fn clear(&self) -> Result<(), HostFailure> {
            self.0.replace(None);
            Ok(())
        }
    }

    #[derive(Default)]
    struct Backend {
        starts: usize,
        fail_start: bool,
        recoveries: usize,
    }

    impl PlatformBackend for Backend {
        fn start(&mut self, _: &StartParameters) -> Result<(), HostFailure> {
            self.starts += 1;
            if self.fail_start {
                Err(HostFailure::Platform)
            } else {
                Ok(())
            }
        }

        fn stop(&mut self) -> Result<(), HostFailure> {
            Ok(())
        }

        fn recover(&mut self, _: &RecoveryRecord) -> Result<(), HostFailure> {
            self.recoveries += 1;
            Ok(())
        }
    }

    fn start(request_id: u64, generation: u64) -> Request {
        Request {
            version: PROTOCOL_VERSION,
            request_id,
            profile_generation: generation,
            capabilities: BTreeSet::from([Capability::Tun]),
            command: Command::Start(StartParameters {
                mtu: 1500,
                ipv4: Some(IpNetwork {
                    address: "172.19.0.1".parse().expect("IP"),
                    prefix_length: 30,
                }),
                ipv6: Some(IpNetwork {
                    address: "fd00:19::1".parse().expect("IP"),
                    prefix_length: 126,
                }),
                dns: vec!["1.1.1.1".parse().expect("DNS")],
            }),
        }
    }

    #[test]
    fn retries_are_idempotent_and_old_generations_are_rejected() {
        let mut host = TrafficHost::new(
            Backend::default(),
            MemoryJournal::default(),
            BTreeSet::from([Capability::Tun]),
        )
        .expect("host");
        let first = host.handle(start(1, 7));
        let retry = host.handle(start(1, 7));
        assert_eq!(first, retry);
        assert_eq!(host.backend.starts, 1);
        assert!(matches!(
            host.handle(start(2, 6)).response,
            CommandResult::Err(ErrorCode::GenerationConflict)
        ));
    }

    #[test]
    fn failed_mutation_requires_recovery_before_another_start() {
        let mut host = TrafficHost::new(
            Backend {
                fail_start: true,
                ..Backend::default()
            },
            MemoryJournal::default(),
            BTreeSet::from([Capability::Tun]),
        )
        .expect("host");
        assert!(matches!(
            host.handle(start(1, 1)).response,
            CommandResult::Err(ErrorCode::Platform)
        ));
        assert!(matches!(
            host.handle(start(2, 2)).response,
            CommandResult::Err(ErrorCode::RecoveryRequired)
        ));
        host.backend.fail_start = false;
        let recovered = host.handle(Request {
            version: PROTOCOL_VERSION,
            request_id: 3,
            profile_generation: 2,
            capabilities: BTreeSet::from([Capability::Tun]),
            command: Command::Recover(EmptyParameters {}),
        });
        assert!(matches!(
            recovered.response,
            CommandResult::Ok(Success::Recovered)
        ));
        assert_eq!(host.state(), HostState::Stopped);
        assert_eq!(host.backend.recoveries, 1);
    }
}

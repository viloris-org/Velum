use velum_client_api::ClientError;

/// Stable lifecycle phases published to platform control consumers.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum RuntimePhase {
    #[default]
    Stopped,
    Connecting,
    Online,
    Stopping,
    Failed,
}

/// Payload-free failure category retained in a runtime snapshot.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeFailure {
    Certificate,
    ConnectTimeout,
    Connection,
    ControlTooLarge,
    DatagramTooLarge,
    DatagramUnavailable,
    Protocol,
    Transport,
}

impl From<ClientError> for RuntimeFailure {
    fn from(error: ClientError) -> Self {
        match error {
            ClientError::Certificate => Self::Certificate,
            ClientError::ConnectTimeout => Self::ConnectTimeout,
            ClientError::Connection => Self::Connection,
            ClientError::ControlTooLarge => Self::ControlTooLarge,
            ClientError::DatagramTooLarge => Self::DatagramTooLarge,
            ClientError::DatagramUnavailable => Self::DatagramUnavailable,
            ClientError::Protocol => Self::Protocol,
            ClientError::Transport => Self::Transport,
        }
    }
}

/// Immutable latest-value view of the client lifecycle.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeSnapshot {
    pub revision: u64,
    pub generation: u64,
    pub phase: RuntimePhase,
    pub failure: Option<RuntimeFailure>,
}

impl Default for RuntimeSnapshot {
    fn default() -> Self {
        Self {
            revision: 0,
            generation: 0,
            phase: RuntimePhase::Stopped,
            failure: None,
        }
    }
}

/// Stable runtime operation errors. No variant retains configuration or data.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeError {
    Busy,
    ExecutorUnavailable,
    NotOnline,
    Superseded,
    Client(ClientError),
}

#[derive(Default)]
pub(crate) struct Lifecycle {
    pub(crate) snapshot: RuntimeSnapshot,
}

impl Lifecycle {
    pub(crate) fn begin_connect(&mut self) -> Result<u64, RuntimeError> {
        if !matches!(
            self.snapshot.phase,
            RuntimePhase::Stopped | RuntimePhase::Failed
        ) {
            return Err(RuntimeError::Busy);
        }
        self.snapshot.generation = self
            .snapshot
            .generation
            .checked_add(1)
            .expect("runtime generation exhausted");
        self.transition(RuntimePhase::Connecting, None);
        Ok(self.snapshot.generation)
    }

    pub(crate) fn complete_connect(
        &mut self,
        generation: u64,
        result: Result<(), RuntimeFailure>,
    ) -> bool {
        if generation != self.snapshot.generation || self.snapshot.phase != RuntimePhase::Connecting
        {
            return false;
        }
        match result {
            Ok(()) => self.transition(RuntimePhase::Online, None),
            Err(failure) => self.transition(RuntimePhase::Failed, Some(failure)),
        }
        true
    }

    pub(crate) fn begin_stop(&mut self) -> Option<u64> {
        if self.snapshot.phase == RuntimePhase::Stopped {
            return None;
        }
        self.snapshot.generation = self
            .snapshot
            .generation
            .checked_add(1)
            .expect("runtime generation exhausted");
        self.transition(RuntimePhase::Stopping, None);
        Some(self.snapshot.generation)
    }

    pub(crate) fn complete_stop(&mut self, generation: u64) -> bool {
        if generation != self.snapshot.generation || self.snapshot.phase != RuntimePhase::Stopping {
            return false;
        }
        self.transition(RuntimePhase::Stopped, None);
        true
    }

    pub(crate) fn fail_online(&mut self, generation: u64, failure: RuntimeFailure) -> bool {
        if generation != self.snapshot.generation || self.snapshot.phase != RuntimePhase::Online {
            return false;
        }
        self.transition(RuntimePhase::Failed, Some(failure));
        true
    }

    fn transition(&mut self, phase: RuntimePhase, failure: Option<RuntimeFailure>) {
        self.snapshot.revision = self
            .snapshot
            .revision
            .checked_add(1)
            .expect("runtime snapshot revision exhausted");
        self.snapshot.phase = phase;
        self.snapshot.failure = failure;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lifecycle_revisions_are_monotonic() {
        let mut lifecycle = Lifecycle::default();
        let generation = lifecycle.begin_connect().expect("begin connect");
        assert_eq!(lifecycle.snapshot.revision, 1);
        assert!(lifecycle.complete_connect(generation, Ok(())));
        assert_eq!(lifecycle.snapshot.revision, 2);
        let stop = lifecycle.begin_stop().expect("begin stop");
        assert_eq!(lifecycle.snapshot.revision, 3);
        assert!(lifecycle.complete_stop(stop));
        assert_eq!(lifecycle.snapshot.revision, 4);
        assert_eq!(lifecycle.snapshot.phase, RuntimePhase::Stopped);
    }

    #[test]
    fn stale_connect_completion_cannot_overwrite_stop() {
        let mut lifecycle = Lifecycle::default();
        let connect = lifecycle.begin_connect().expect("begin connect");
        let stop = lifecycle.begin_stop().expect("begin stop");
        assert!(lifecycle.complete_stop(stop));
        let stopped = lifecycle.snapshot;

        assert!(!lifecycle.complete_connect(connect, Ok(())));
        assert_eq!(lifecycle.snapshot, stopped);
    }

    #[test]
    fn failed_connection_can_be_retried_with_a_new_generation() {
        let mut lifecycle = Lifecycle::default();
        let first = lifecycle.begin_connect().expect("first connect");
        assert!(lifecycle.complete_connect(first, Err(RuntimeFailure::Connection)));
        assert_eq!(lifecycle.snapshot.phase, RuntimePhase::Failed);
        assert_eq!(lifecycle.snapshot.failure, Some(RuntimeFailure::Connection));

        let second = lifecycle.begin_connect().expect("retry connect");
        assert!(second > first);
        assert_eq!(lifecycle.snapshot.phase, RuntimePhase::Connecting);
        assert_eq!(lifecycle.snapshot.failure, None);
    }

    #[test]
    fn busy_lifecycle_rejects_parallel_connect() {
        let mut lifecycle = Lifecycle::default();
        lifecycle.begin_connect().expect("first connect");
        assert_eq!(lifecycle.begin_connect(), Err(RuntimeError::Busy));
    }

    #[test]
    fn active_generation_records_transport_closure_as_failure() {
        let mut lifecycle = Lifecycle::default();
        let generation = lifecycle.begin_connect().expect("begin connect");
        assert!(lifecycle.complete_connect(generation, Ok(())));

        assert!(lifecycle.fail_online(generation, RuntimeFailure::Connection));
        assert_eq!(lifecycle.snapshot.phase, RuntimePhase::Failed);
        assert_eq!(lifecycle.snapshot.failure, Some(RuntimeFailure::Connection));
        assert!(!lifecycle.fail_online(generation, RuntimeFailure::Transport));
    }
}

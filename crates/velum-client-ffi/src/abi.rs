use velum_client_runtime::{RuntimeFailure, RuntimePhase, RuntimeSnapshot};

/// Stable version for the original synchronous Flutter native ABI.
pub const ABI_VERSION: u16 = 2;

/// Stable version for the additive asynchronous runtime control ABI.
pub const RUNTIME_ABI_VERSION: u16 = 2;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct VelumByteSlice {
    pub pointer: *const u8,
    pub length: usize,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct VelumMutableByteSlice {
    pub pointer: *mut u8,
    pub length: usize,
}

/// Configuration copied by native code before a start call returns.
#[repr(C)]
pub struct VelumClientConfigInput {
    pub relay_address: VelumByteSlice,
    pub server_name: VelumByteSlice,
    pub credential: VelumByteSlice,
    pub certificate_pem: VelumByteSlice,
    pub connect_timeout_millis: u64,
    pub trust_mode: u32,
}

pub const VELUM_TRUST_SYSTEM: u32 = 0;
pub const VELUM_TRUST_CUSTOM_CA: u32 = 1;
pub const VELUM_TRUST_INSECURE: u32 = 2;

/// Status values for synchronous stream operations in ABI v2.
#[repr(i32)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VelumStatus {
    Ok = 0,
    InvalidArgument = 1,
    InvalidHandle = 2,
    Configuration = 3,
    Certificate = 4,
    ConnectTimeout = 5,
    Connection = 6,
    ControlTooLarge = 7,
    Transport = 8,
    DatagramTooLarge = 9,
    DatagramUnavailable = 10,
    Protocol = 11,
}

/// Status values for immediate runtime control command handling.
#[repr(i32)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VelumControlStatus {
    Ok = 0,
    InvalidArgument = 1,
    InvalidHandle = 2,
    Configuration = 3,
    Certificate = 4,
    Busy = 5,
    Internal = 6,
}

#[repr(u32)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VelumRuntimePhase {
    Stopped = 0,
    Connecting = 1,
    Online = 2,
    Stopping = 3,
    Failed = 4,
}

#[repr(u32)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VelumRuntimeFailure {
    None = 0,
    Certificate = 1,
    ConnectTimeout = 2,
    Connection = 3,
    ControlTooLarge = 4,
    DatagramTooLarge = 5,
    DatagramUnavailable = 6,
    Protocol = 7,
    Transport = 8,
}

/// Fixed-width latest-value snapshot for runtime ABI v2.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct VelumRuntimeSnapshotV1 {
    pub revision: u64,
    pub generation: u64,
    pub phase: u32,
    pub failure: u32,
}

impl From<RuntimeSnapshot> for VelumRuntimeSnapshotV1 {
    fn from(snapshot: RuntimeSnapshot) -> Self {
        Self {
            revision: snapshot.revision,
            generation: snapshot.generation,
            phase: match snapshot.phase {
                RuntimePhase::Stopped => VelumRuntimePhase::Stopped as u32,
                RuntimePhase::Connecting => VelumRuntimePhase::Connecting as u32,
                RuntimePhase::Online => VelumRuntimePhase::Online as u32,
                RuntimePhase::Stopping => VelumRuntimePhase::Stopping as u32,
                RuntimePhase::Failed => VelumRuntimePhase::Failed as u32,
            },
            failure: snapshot
                .failure
                .map_or(VelumRuntimeFailure::None as u32, |failure| match failure {
                    RuntimeFailure::Certificate => VelumRuntimeFailure::Certificate as u32,
                    RuntimeFailure::ConnectTimeout => VelumRuntimeFailure::ConnectTimeout as u32,
                    RuntimeFailure::Connection => VelumRuntimeFailure::Connection as u32,
                    RuntimeFailure::ControlTooLarge => VelumRuntimeFailure::ControlTooLarge as u32,
                    RuntimeFailure::DatagramTooLarge => {
                        VelumRuntimeFailure::DatagramTooLarge as u32
                    }
                    RuntimeFailure::DatagramUnavailable => {
                        VelumRuntimeFailure::DatagramUnavailable as u32
                    }
                    RuntimeFailure::Protocol => VelumRuntimeFailure::Protocol as u32,
                    RuntimeFailure::Transport => VelumRuntimeFailure::Transport as u32,
                }),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::mem::{align_of, offset_of, size_of};

    use super::*;

    #[test]
    fn runtime_snapshot_v1_has_the_documented_layout() {
        assert_eq!(size_of::<VelumRuntimeSnapshotV1>(), 24);
        assert_eq!(offset_of!(VelumRuntimeSnapshotV1, revision), 0);
        assert_eq!(offset_of!(VelumRuntimeSnapshotV1, generation), 8);
        assert_eq!(offset_of!(VelumRuntimeSnapshotV1, phase), 16);
        assert_eq!(offset_of!(VelumRuntimeSnapshotV1, failure), 20);
    }

    #[test]
    fn published_numeric_values_are_stable() {
        assert_eq!(size_of::<VelumStatus>(), 4);
        assert_eq!(align_of::<VelumStatus>(), 4);
        assert_eq!(VelumStatus::Protocol as i32, 11);
        assert_eq!(VelumControlStatus::Internal as i32, 6);
        assert_eq!(VelumRuntimePhase::Failed as u32, 4);
        assert_eq!(VelumRuntimeFailure::Transport as u32, 8);
    }
}

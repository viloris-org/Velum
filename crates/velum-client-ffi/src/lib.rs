//! Reviewed native ABI for Flutter's direct Velum client API.
//!
//! The ABI exposes numeric handles only. Inputs are copied before native work
//! begins, outputs are written only during the call, and no caller pointer is
//! retained. ABI v2 provides synchronous stream operations and non-blocking
//! runtime lifecycle control. ABI v1 is retired during the internal test phase.

mod abi;
mod configuration;
mod control;
mod handles;
mod streams;

use std::sync::OnceLock;

pub use abi::*;
pub use control::*;
pub use streams::*;
use tokio::runtime::{Builder, Runtime};
use velum_client_runtime::{ClientError, RuntimeError};

pub(crate) fn executor() -> &'static Runtime {
    static RUNTIME: OnceLock<Runtime> = OnceLock::new();
    RUNTIME.get_or_init(|| {
        Builder::new_multi_thread()
            .enable_io()
            .enable_time()
            .build()
            .expect("native client runtime")
    })
}

pub(crate) fn status_for_client(error: ClientError) -> VelumStatus {
    match error {
        ClientError::Certificate => VelumStatus::Certificate,
        ClientError::ConnectTimeout => VelumStatus::ConnectTimeout,
        ClientError::Connection => VelumStatus::Connection,
        ClientError::ControlTooLarge => VelumStatus::ControlTooLarge,
        ClientError::DatagramTooLarge => VelumStatus::DatagramTooLarge,
        ClientError::DatagramUnavailable => VelumStatus::DatagramUnavailable,
        ClientError::Protocol => VelumStatus::Protocol,
        ClientError::Transport => VelumStatus::Transport,
    }
}

pub(crate) fn status_for_runtime(error: RuntimeError) -> VelumStatus {
    match error {
        RuntimeError::Client(error) => status_for_client(error),
        RuntimeError::Busy
        | RuntimeError::ExecutorUnavailable
        | RuntimeError::NotOnline
        | RuntimeError::Superseded => VelumStatus::Transport,
    }
}

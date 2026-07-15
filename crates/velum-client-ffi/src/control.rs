use std::sync::Arc;

use velum_client_runtime::{
    ClientConfig, ClientRuntime, RuntimeError, RuntimeFailure, RuntimePhase,
};

use crate::{
    ABI_VERSION, RUNTIME_ABI_VERSION, VelumClientConfigInput, VelumControlStatus,
    VelumRuntimeSnapshotV1, VelumStatus,
    configuration::{configuration_from_input, copy_bytes},
    executor,
    handles::{ClientEntry, handles},
};
use velum_adapter_proxy::{ProxyAdapter, RoutingPolicy};

enum SynchronousConnectError {
    Runtime(RuntimeError),
    Failure(RuntimeFailure),
}

enum RetireError {
    InvalidHandle,
    Internal,
}

/// Returns the synchronous ABI version supported by this library.
#[unsafe(no_mangle)]
pub extern "C" fn velum_client_abi_version() -> u16 {
    ABI_VERSION
}

/// Returns the asynchronous runtime ABI version supported by this library.
#[unsafe(no_mangle)]
pub extern "C" fn velum_client_runtime_abi_version() -> u16 {
    RUNTIME_ABI_VERSION
}

/// Creates one stopped runtime and returns its opaque numeric handle.
///
/// # Safety
///
/// `out_runtime_handle` must be writable for the duration of this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn velum_client_runtime_create(
    out_runtime_handle: *mut u64,
) -> VelumControlStatus {
    if out_runtime_handle.is_null() {
        return VelumControlStatus::InvalidArgument;
    }
    let entry = Arc::new(ClientEntry::new(Arc::new(ClientRuntime::new())));
    let handle = match handles().lock() {
        Ok(mut table) => match table.insert_client(entry) {
            Some(handle) => handle,
            None => return VelumControlStatus::Internal,
        },
        Err(_) => return VelumControlStatus::Internal,
    };
    unsafe { *out_runtime_handle = handle };
    VelumControlStatus::Ok
}

/// Accepts a start command and returns its visible connection generation.
///
/// All configuration bytes are copied before this call returns. Network
/// establishment continues on the native runtime.
///
/// # Safety
///
/// `input` and `out_generation` must be valid for this call. Every nonempty
/// byte slice in `input` must be readable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn velum_client_runtime_start_v1(
    runtime_handle: u64,
    input: *const VelumClientConfigInput,
    out_generation: *mut u64,
) -> VelumControlStatus {
    if out_generation.is_null() {
        return VelumControlStatus::InvalidArgument;
    }
    let configuration = match unsafe { configuration_from_input(input) } {
        Ok(configuration) => configuration,
        Err(error) => return error.into(),
    };
    let entry = match handles().lock() {
        Ok(table) => table.clients.get(&runtime_handle).cloned(),
        Err(_) => return VelumControlStatus::Internal,
    };
    let Some(entry) = entry else {
        return VelumControlStatus::InvalidHandle;
    };
    let command = match entry.command.lock() {
        Ok(command) => command,
        Err(_) => return VelumControlStatus::Internal,
    };
    if command.destroyed {
        return VelumControlStatus::InvalidHandle;
    }
    let generation = match executor().block_on(entry.runtime.start(configuration)) {
        Ok(generation) => generation,
        Err(RuntimeError::Busy) => return VelumControlStatus::Busy,
        Err(
            RuntimeError::ExecutorUnavailable
            | RuntimeError::NotOnline
            | RuntimeError::Superseded
            | RuntimeError::Client(_),
        ) => return VelumControlStatus::Internal,
    };
    unsafe { *out_generation = generation };
    VelumControlStatus::Ok
}

/// Copies the latest immutable runtime snapshot.
///
/// # Safety
///
/// `out_snapshot` must be writable for the duration of this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn velum_client_runtime_snapshot_v1(
    runtime_handle: u64,
    out_snapshot: *mut VelumRuntimeSnapshotV1,
) -> VelumControlStatus {
    if out_snapshot.is_null() {
        return VelumControlStatus::InvalidArgument;
    }
    let entry = match handles().lock() {
        Ok(table) => table.clients.get(&runtime_handle).cloned(),
        Err(_) => return VelumControlStatus::Internal,
    };
    let Some(entry) = entry else {
        return VelumControlStatus::InvalidHandle;
    };
    let command = match entry.command.lock() {
        Ok(command) => command,
        Err(_) => return VelumControlStatus::Internal,
    };
    if command.destroyed {
        return VelumControlStatus::InvalidHandle;
    }
    unsafe { *out_snapshot = entry.runtime.snapshot().into() };
    VelumControlStatus::Ok
}

/// Stops one runtime while retaining its handle for a later start.
///
/// # Safety
///
/// `out_generation` must be writable for the duration of this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn velum_client_runtime_stop(
    runtime_handle: u64,
    out_generation: *mut u64,
) -> VelumControlStatus {
    if out_generation.is_null() {
        return VelumControlStatus::InvalidArgument;
    }
    let entry = match handles().lock() {
        Ok(table) => table.clients.get(&runtime_handle).cloned(),
        Err(_) => return VelumControlStatus::Internal,
    };
    let Some(entry) = entry else {
        return VelumControlStatus::InvalidHandle;
    };
    let command = match entry.command.lock() {
        Ok(command) => command,
        Err(_) => return VelumControlStatus::Internal,
    };
    if command.destroyed {
        return VelumControlStatus::InvalidHandle;
    }
    if stop_proxy(&entry).is_err() {
        return VelumControlStatus::Internal;
    }
    match handles().lock() {
        Ok(mut table) => table.invalidate_streams(runtime_handle),
        Err(_) => return VelumControlStatus::Internal,
    }
    let generation = executor().block_on(entry.runtime.stop());
    unsafe { *out_generation = generation };
    VelumControlStatus::Ok
}

/// Starts the loopback-only local proxy for an online runtime.
///
/// The proxy accepts HTTP CONNECT and SOCKS5 requests containing literal IP
/// destinations only. Its port is returned only after the listener is bound.
///
/// # Safety
///
/// `out_port` must be writable for the duration of this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn velum_client_runtime_proxy_start(
    runtime_handle: u64,
    requested_port: u16,
    out_port: *mut u16,
) -> VelumControlStatus {
    unsafe {
        start_proxy_with_policy(
            runtime_handle,
            requested_port,
            RoutingPolicy::default(),
            out_port,
        )
    }
}

/// Starts the loopback proxy with an ordered UTF-8 routing policy.
///
/// # Safety
///
/// `rules` must be readable and `out_port` writable for this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn velum_client_runtime_proxy_start_v2(
    runtime_handle: u64,
    requested_port: u16,
    rules: crate::VelumByteSlice,
    out_port: *mut u16,
) -> VelumControlStatus {
    let rules = match unsafe { copy_bytes(rules) } {
        Ok(rules) => rules,
        Err(error) => return error.into(),
    };
    let rules = match std::str::from_utf8(&rules) {
        Ok(rules) => rules,
        Err(_) => return VelumControlStatus::Configuration,
    };
    let policy = match rules.parse::<RoutingPolicy>() {
        Ok(policy) if !policy.rules().is_empty() => policy,
        Ok(_) | Err(_) => return VelumControlStatus::Configuration,
    };
    unsafe { start_proxy_with_policy(runtime_handle, requested_port, policy, out_port) }
}

unsafe fn start_proxy_with_policy(
    runtime_handle: u64,
    requested_port: u16,
    policy: RoutingPolicy,
    out_port: *mut u16,
) -> VelumControlStatus {
    if out_port.is_null() {
        return VelumControlStatus::InvalidArgument;
    }
    let entry = match handles().lock() {
        Ok(table) => table.clients.get(&runtime_handle).cloned(),
        Err(_) => return VelumControlStatus::Internal,
    };
    let Some(entry) = entry else {
        return VelumControlStatus::InvalidHandle;
    };
    let command = match entry.command.lock() {
        Ok(command) => command,
        Err(_) => return VelumControlStatus::Internal,
    };
    if command.destroyed {
        return VelumControlStatus::InvalidHandle;
    }
    let mut proxy = match entry.proxy.lock() {
        Ok(proxy) => proxy,
        Err(_) => return VelumControlStatus::Internal,
    };
    if proxy.is_some() {
        return VelumControlStatus::Busy;
    }
    let adapter = match executor().block_on(ProxyAdapter::start_with_policy(
        Arc::clone(&entry.runtime),
        requested_port,
        policy,
    )) {
        Ok(adapter) => adapter,
        Err(_) => return VelumControlStatus::Internal,
    };
    unsafe { *out_port = adapter.address().port() };
    *proxy = Some(adapter);
    VelumControlStatus::Ok
}

/// Stops the runtime's loopback proxy without stopping the QUIC runtime.
#[unsafe(no_mangle)]
pub extern "C" fn velum_client_runtime_proxy_stop(runtime_handle: u64) -> VelumControlStatus {
    let entry = match handles().lock() {
        Ok(table) => table.clients.get(&runtime_handle).cloned(),
        Err(_) => return VelumControlStatus::Internal,
    };
    let Some(entry) = entry else {
        return VelumControlStatus::InvalidHandle;
    };
    let command = match entry.command.lock() {
        Ok(command) => command,
        Err(_) => return VelumControlStatus::Internal,
    };
    if command.destroyed {
        return VelumControlStatus::InvalidHandle;
    }
    match stop_proxy(&entry) {
        Ok(()) => VelumControlStatus::Ok,
        Err(_) => VelumControlStatus::Internal,
    }
}

/// Destroys a runtime and invalidates every handle derived from it.
#[unsafe(no_mangle)]
pub extern "C" fn velum_client_runtime_destroy(runtime_handle: u64) -> VelumControlStatus {
    match retire_runtime(runtime_handle) {
        Ok(()) => VelumControlStatus::Ok,
        Err(RetireError::InvalidHandle) => VelumControlStatus::InvalidHandle,
        Err(RetireError::Internal) => VelumControlStatus::Internal,
    }
}

/// Creates and connects a direct client using synchronous ABI v2.
///
/// # Safety
///
/// `input` and `out_client_handle` must be valid pointers for the duration of
/// this call. Each nonempty byte slice in `input` must be readable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn velum_client_connect(
    input: *const VelumClientConfigInput,
    out_client_handle: *mut u64,
) -> VelumStatus {
    if out_client_handle.is_null() {
        return VelumStatus::InvalidArgument;
    }
    let configuration = match unsafe { configuration_from_input(input) } {
        Ok(configuration) => configuration,
        Err(error) => return error.into(),
    };
    let runtime = Arc::new(ClientRuntime::new());
    match executor().block_on(wait_until_connected(Arc::clone(&runtime), configuration)) {
        Ok(()) => {}
        Err(SynchronousConnectError::Runtime(error)) => return crate::status_for_runtime(error),
        Err(SynchronousConnectError::Failure(failure)) => return status_for_failure(failure),
    }
    let entry = Arc::new(ClientEntry::new(runtime));
    let handle = match handles().lock() {
        Ok(mut table) => match table.insert_client(entry) {
            Some(handle) => handle,
            None => return VelumStatus::Transport,
        },
        Err(_) => return VelumStatus::Transport,
    };
    unsafe { *out_client_handle = handle };
    VelumStatus::Ok
}

/// Closes a synchronous ABI v2 client and invalidates all derived streams.
#[unsafe(no_mangle)]
pub extern "C" fn velum_client_close(client_handle: u64) -> VelumStatus {
    match retire_runtime(client_handle) {
        Ok(()) => VelumStatus::Ok,
        Err(RetireError::InvalidHandle) => VelumStatus::InvalidHandle,
        Err(RetireError::Internal) => VelumStatus::Transport,
    }
}

async fn wait_until_connected(
    runtime: Arc<ClientRuntime>,
    configuration: ClientConfig,
) -> Result<(), SynchronousConnectError> {
    let mut snapshots = runtime.subscribe();
    runtime
        .start(configuration)
        .await
        .map_err(SynchronousConnectError::Runtime)?;
    loop {
        let snapshot = *snapshots.borrow();
        match snapshot.phase {
            RuntimePhase::Online => return Ok(()),
            RuntimePhase::Failed => {
                return Err(SynchronousConnectError::Failure(
                    snapshot.failure.unwrap_or(RuntimeFailure::Transport),
                ));
            }
            RuntimePhase::Stopped | RuntimePhase::Stopping => {
                return Err(SynchronousConnectError::Runtime(RuntimeError::Superseded));
            }
            RuntimePhase::Connecting => snapshots
                .changed()
                .await
                .map_err(|_| SynchronousConnectError::Runtime(RuntimeError::Superseded))?,
        }
    }
}

fn retire_runtime(runtime_handle: u64) -> Result<(), RetireError> {
    let entry = match handles().lock() {
        Ok(mut table) => {
            table.invalidate_streams(runtime_handle);
            table.clients.remove(&runtime_handle)
        }
        Err(_) => return Err(RetireError::Internal),
    };
    let Some(entry) = entry else {
        return Err(RetireError::InvalidHandle);
    };
    let mut command = entry.command.lock().map_err(|_| RetireError::Internal)?;
    command.destroyed = true;
    stop_proxy(&entry).map_err(|_| RetireError::Internal)?;
    executor().block_on(entry.runtime.stop());
    Ok(())
}

fn stop_proxy(entry: &ClientEntry) -> Result<(), RetireError> {
    let mut proxy = entry.proxy.lock().map_err(|_| RetireError::Internal)?;
    if let Some(adapter) = proxy.take() {
        adapter.stop();
    }
    Ok(())
}

fn status_for_failure(failure: RuntimeFailure) -> VelumStatus {
    match failure {
        RuntimeFailure::Certificate => VelumStatus::Certificate,
        RuntimeFailure::ConnectTimeout => VelumStatus::ConnectTimeout,
        RuntimeFailure::Connection => VelumStatus::Connection,
        RuntimeFailure::ControlTooLarge => VelumStatus::ControlTooLarge,
        RuntimeFailure::DatagramTooLarge => VelumStatus::DatagramTooLarge,
        RuntimeFailure::DatagramUnavailable => VelumStatus::DatagramUnavailable,
        RuntimeFailure::Protocol => VelumStatus::Protocol,
        RuntimeFailure::Transport => VelumStatus::Transport,
    }
}

#[cfg(test)]
mod tests {
    use rcgen::CertifiedKey;

    use super::*;

    fn byte_slice(bytes: &[u8]) -> crate::VelumByteSlice {
        crate::VelumByteSlice {
            pointer: bytes.as_ptr(),
            length: bytes.len(),
        }
    }

    #[test]
    fn v2_versions_and_invalid_handles_remain_stable() {
        assert_eq!(velum_client_abi_version(), 2);
        assert_eq!(velum_client_runtime_abi_version(), 2);
        assert_eq!(velum_client_close(u64::MAX), VelumStatus::InvalidHandle);
        assert_eq!(
            velum_client_runtime_destroy(u64::MAX),
            VelumControlStatus::InvalidHandle
        );
    }

    #[test]
    fn create_snapshot_stop_and_destroy_form_a_reusable_lifecycle() {
        let mut handle = 0;
        assert_eq!(
            unsafe { velum_client_runtime_create(&mut handle) },
            VelumControlStatus::Ok
        );
        let mut snapshot = VelumRuntimeSnapshotV1::default();
        assert_eq!(
            unsafe { velum_client_runtime_snapshot_v1(handle, &mut snapshot) },
            VelumControlStatus::Ok
        );
        assert_eq!(snapshot.phase, crate::VelumRuntimePhase::Stopped as u32);
        assert_eq!(snapshot.revision, 0);
        assert_eq!(snapshot.generation, 0);

        let mut generation = u64::MAX;
        assert_eq!(
            unsafe { velum_client_runtime_stop(handle, &mut generation) },
            VelumControlStatus::Ok
        );
        assert_eq!(generation, 0);
        assert_eq!(velum_client_runtime_destroy(handle), VelumControlStatus::Ok);
        assert_eq!(
            unsafe { velum_client_runtime_snapshot_v1(handle, &mut snapshot) },
            VelumControlStatus::InvalidHandle
        );
    }

    #[test]
    fn null_outputs_are_rejected_without_mutating_global_state() {
        assert_eq!(
            unsafe { velum_client_runtime_create(std::ptr::null_mut()) },
            VelumControlStatus::InvalidArgument
        );
        assert_eq!(
            unsafe { velum_client_connect(std::ptr::null(), std::ptr::null_mut()) },
            VelumStatus::InvalidArgument
        );
    }

    #[test]
    fn runtime_start_copies_inputs_and_stop_does_not_wait_for_network_timeout() {
        let CertifiedKey { cert, .. } =
            rcgen::generate_simple_self_signed(vec!["localhost".into()]).expect("certificate");
        let mut relay_address = b"192.0.2.1:443".to_vec();
        let mut server_name = b"localhost".to_vec();
        let mut credential = vec![7_u8; 32];
        let mut certificate_pem = cert.pem().into_bytes();
        let input = crate::VelumClientConfigInput {
            relay_address: byte_slice(&relay_address),
            server_name: byte_slice(&server_name),
            credential: byte_slice(&credential),
            certificate_pem: byte_slice(&certificate_pem),
            connect_timeout_millis: 60_000,
            trust_mode: crate::VELUM_TRUST_CUSTOM_CA,
        };
        let mut handle = 0;
        assert_eq!(
            unsafe { velum_client_runtime_create(&mut handle) },
            VelumControlStatus::Ok
        );
        let mut start_generation = 0;
        assert_eq!(
            unsafe { velum_client_runtime_start_v1(handle, &input, &mut start_generation) },
            VelumControlStatus::Ok
        );
        assert!(start_generation > 0);

        relay_address.fill(0);
        server_name.fill(0);
        credential.fill(0);
        certificate_pem.fill(0);

        let mut stop_generation = 0;
        assert_eq!(
            unsafe { velum_client_runtime_stop(handle, &mut stop_generation) },
            VelumControlStatus::Ok
        );
        assert!(stop_generation > start_generation);
        let mut snapshot = VelumRuntimeSnapshotV1::default();
        assert_eq!(
            unsafe { velum_client_runtime_snapshot_v1(handle, &mut snapshot) },
            VelumControlStatus::Ok
        );
        assert_eq!(snapshot.generation, stop_generation);
        assert_eq!(snapshot.phase, crate::VelumRuntimePhase::Stopped as u32);
        assert_eq!(velum_client_runtime_destroy(handle), VelumControlStatus::Ok);
    }
}

use std::{slice, sync::Arc};

use velum_adapter_proxy::{ProxyAdapter, RoutingPolicy};
use velum_client_engine::{NodePool, NodePoolError, ResolvedNode};
use velum_client_profile::MAX_NODES;

use crate::{
    ENGINE_ABI_VERSION, VelumByteSlice, VelumControlStatus, VelumEngineNodeInput,
    VelumEngineNodeSnapshotV1, VelumRuntimeSnapshotV1,
    configuration::{configuration_from_input, copy_bytes},
    executor,
    handles::{EngineEntry, handles},
};

/// Returns the multi-node engine ABI version supported by this library.
#[unsafe(no_mangle)]
pub extern "C" fn velum_client_engine_abi_version() -> u16 {
    ENGINE_ABI_VERSION
}

/// Creates one stopped multi-node engine and returns its opaque handle.
///
/// # Safety
///
/// `out_engine_handle` must be writable for the duration of this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn velum_client_engine_create(
    out_engine_handle: *mut u64,
) -> VelumControlStatus {
    if out_engine_handle.is_null() {
        return VelumControlStatus::InvalidArgument;
    }
    let entry = Arc::new(EngineEntry::new(Arc::new(NodePool::default())));
    let handle = match handles().lock() {
        Ok(mut table) => match table.insert_engine(entry) {
            Some(handle) => handle,
            None => return VelumControlStatus::Internal,
        },
        Err(_) => return VelumControlStatus::Internal,
    };
    unsafe { *out_engine_handle = handle };
    VelumControlStatus::Ok
}

/// Replaces the active profile generation and starts the default node without
/// waiting for network establishment.
///
/// # Safety
///
/// `nodes`, `default_node`, and `out_generation` must remain valid for this
/// call. Every nonempty byte slice in each node must be readable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn velum_client_engine_activate_v1(
    engine_handle: u64,
    nodes: *const VelumEngineNodeInput,
    node_count: usize,
    default_node: VelumByteSlice,
    out_generation: *mut u64,
) -> VelumControlStatus {
    if nodes.is_null() || out_generation.is_null() || node_count == 0 || node_count > MAX_NODES {
        return VelumControlStatus::InvalidArgument;
    }
    let default_node = match unsafe { text(default_node) } {
        Ok(value) => value,
        Err(status) => return status,
    };
    let inputs = unsafe { slice::from_raw_parts(nodes, node_count) };
    let mut resolved = Vec::with_capacity(node_count);
    for input in inputs {
        let id = match unsafe { text(input.id) } {
            Ok(value) => value,
            Err(status) => return status,
        };
        let alias = match unsafe { text(input.alias) } {
            Ok(value) => value,
            Err(status) => return status,
        };
        let configuration = match unsafe { configuration_from_input(&input.configuration) } {
            Ok(configuration) => configuration,
            Err(error) => return error.into(),
        };
        resolved.push(ResolvedNode {
            id,
            alias,
            configuration,
        });
    }
    let entry = match engine_entry(engine_handle) {
        Ok(entry) => entry,
        Err(status) => return status,
    };
    let command = match entry.command.lock() {
        Ok(command) if !command.destroyed => command,
        Ok(_) => return VelumControlStatus::InvalidHandle,
        Err(_) => return VelumControlStatus::Internal,
    };
    if stop_proxy(&entry).is_err() {
        return VelumControlStatus::Internal;
    }
    let generation = match executor().block_on(entry.pool.activate(resolved, &default_node)) {
        Ok(generation) => generation,
        Err(error) => return status_for_engine(error),
    };
    drop(command);
    unsafe { *out_generation = generation };
    VelumControlStatus::Ok
}

/// Copies the latest snapshot for one configured node without starting it.
///
/// `reference` may be the stable ID or alias known to the caller.
///
/// # Safety
///
/// `reference` must be readable and `out_snapshot` writable for this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn velum_client_engine_node_snapshot_v1(
    engine_handle: u64,
    reference: VelumByteSlice,
    out_snapshot: *mut VelumEngineNodeSnapshotV1,
) -> VelumControlStatus {
    if out_snapshot.is_null() {
        return VelumControlStatus::InvalidArgument;
    }
    let reference = match unsafe { text(reference) } {
        Ok(value) => value,
        Err(status) => return status,
    };
    let entry = match engine_entry(engine_handle) {
        Ok(entry) => entry,
        Err(status) => return status,
    };
    let command = match entry.command.lock() {
        Ok(command) if !command.destroyed => command,
        Ok(_) => return VelumControlStatus::InvalidHandle,
        Err(_) => return VelumControlStatus::Internal,
    };
    let snapshot = executor().block_on(entry.pool.snapshot());
    let Some(node) = snapshot
        .nodes
        .iter()
        .find(|node| node.id == reference || node.alias == reference)
    else {
        return VelumControlStatus::Configuration;
    };
    unsafe {
        *out_snapshot = VelumEngineNodeSnapshotV1 {
            profile_generation: snapshot.generation,
            configured: 1,
            is_default: u32::from(node.is_default),
            runtime: node
                .runtime
                .map_or_else(VelumRuntimeSnapshotV1::default, Into::into),
        };
    }
    drop(command);
    VelumControlStatus::Ok
}

/// Stops every node in the active profile generation while retaining the handle.
///
/// # Safety
///
/// `out_generation` must be writable for the duration of this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn velum_client_engine_stop(
    engine_handle: u64,
    out_generation: *mut u64,
) -> VelumControlStatus {
    if out_generation.is_null() {
        return VelumControlStatus::InvalidArgument;
    }
    let entry = match engine_entry(engine_handle) {
        Ok(entry) => entry,
        Err(status) => return status,
    };
    let command = match entry.command.lock() {
        Ok(command) if !command.destroyed => command,
        Ok(_) => return VelumControlStatus::InvalidHandle,
        Err(_) => return VelumControlStatus::Internal,
    };
    if stop_proxy(&entry).is_err() {
        return VelumControlStatus::Internal;
    }
    let generation = executor().block_on(entry.pool.stop());
    drop(command);
    unsafe { *out_generation = generation };
    VelumControlStatus::Ok
}

/// Destroys an engine and stops every runtime it owns.
#[unsafe(no_mangle)]
pub extern "C" fn velum_client_engine_destroy(engine_handle: u64) -> VelumControlStatus {
    let entry = match handles().lock() {
        Ok(mut table) => table.engines.remove(&engine_handle),
        Err(_) => return VelumControlStatus::Internal,
    };
    let Some(entry) = entry else {
        return VelumControlStatus::InvalidHandle;
    };
    let mut command = match entry.command.lock() {
        Ok(command) => command,
        Err(_) => return VelumControlStatus::Internal,
    };
    command.destroyed = true;
    if stop_proxy(&entry).is_err() {
        return VelumControlStatus::Internal;
    }
    executor().block_on(entry.pool.stop());
    VelumControlStatus::Ok
}

/// Starts the loopback proxy backed by the active multi-node engine.
///
/// # Safety
///
/// `rules` must be readable and `out_port` writable for this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn velum_client_engine_proxy_start_v1(
    engine_handle: u64,
    requested_port: u16,
    rules: VelumByteSlice,
    out_port: *mut u16,
) -> VelumControlStatus {
    if out_port.is_null() {
        return VelumControlStatus::InvalidArgument;
    }
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
    let entry = match engine_entry(engine_handle) {
        Ok(entry) => entry,
        Err(status) => return status,
    };
    let command = match entry.command.lock() {
        Ok(command) if !command.destroyed => command,
        Ok(_) => return VelumControlStatus::InvalidHandle,
        Err(_) => return VelumControlStatus::Internal,
    };
    let mut proxy = match entry.proxy.lock() {
        Ok(proxy) => proxy,
        Err(_) => return VelumControlStatus::Internal,
    };
    if proxy.is_some() {
        return VelumControlStatus::Busy;
    }
    let adapter = match executor().block_on(ProxyAdapter::start_with_pool_policy(
        Arc::clone(&entry.pool),
        requested_port,
        policy,
    )) {
        Ok(adapter) => adapter,
        Err(_) => return VelumControlStatus::Internal,
    };
    unsafe { *out_port = adapter.address().port() };
    *proxy = Some(adapter);
    drop(command);
    VelumControlStatus::Ok
}

/// Stops the engine-backed loopback proxy without stopping node runtimes.
#[unsafe(no_mangle)]
pub extern "C" fn velum_client_engine_proxy_stop(engine_handle: u64) -> VelumControlStatus {
    let entry = match engine_entry(engine_handle) {
        Ok(entry) => entry,
        Err(status) => return status,
    };
    let command = match entry.command.lock() {
        Ok(command) if !command.destroyed => command,
        Ok(_) => return VelumControlStatus::InvalidHandle,
        Err(_) => return VelumControlStatus::Internal,
    };
    let result = stop_proxy(&entry);
    drop(command);
    match result {
        Ok(()) => VelumControlStatus::Ok,
        Err(()) => VelumControlStatus::Internal,
    }
}

fn engine_entry(engine_handle: u64) -> Result<Arc<EngineEntry>, VelumControlStatus> {
    let entry = match handles().lock() {
        Ok(table) => table.engines.get(&engine_handle).cloned(),
        Err(_) => return Err(VelumControlStatus::Internal),
    };
    let Some(entry) = entry else {
        return Err(VelumControlStatus::InvalidHandle);
    };
    Ok(entry)
}

unsafe fn text(value: VelumByteSlice) -> Result<String, VelumControlStatus> {
    let bytes = unsafe { copy_bytes(value) }.map_err(VelumControlStatus::from)?;
    let value = std::str::from_utf8(&bytes).map_err(|_| VelumControlStatus::InvalidArgument)?;
    if value.is_empty() || value.len() > 128 {
        return Err(VelumControlStatus::Configuration);
    }
    Ok(value.to_owned())
}

fn status_for_engine(error: NodePoolError) -> VelumControlStatus {
    match error {
        NodePoolError::Empty
        | NodePoolError::DuplicateNode
        | NodePoolError::MissingDefault
        | NodePoolError::UnknownNode => VelumControlStatus::Configuration,
        NodePoolError::Superseded | NodePoolError::ConnectionFailed | NodePoolError::Runtime(_) => {
            VelumControlStatus::Internal
        }
    }
}

fn stop_proxy(entry: &EngineEntry) -> Result<(), ()> {
    let mut proxy = entry.proxy.lock().map_err(|_| ())?;
    if let Some(adapter) = proxy.take() {
        adapter.stop();
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn slice(value: &[u8]) -> VelumByteSlice {
        VelumByteSlice {
            pointer: value.as_ptr(),
            length: value.len(),
        }
    }

    #[test]
    fn engine_abi_rejects_invalid_handles_and_releases_destroyed_handles() {
        assert_eq!(velum_client_engine_abi_version(), 1);
        assert_eq!(
            velum_client_engine_destroy(u64::MAX),
            VelumControlStatus::InvalidHandle
        );

        let mut handle = 0;
        assert_eq!(
            unsafe { velum_client_engine_create(&mut handle) },
            VelumControlStatus::Ok
        );
        let mut generation = 0;
        assert_eq!(
            unsafe { velum_client_engine_stop(handle, &mut generation) },
            VelumControlStatus::Ok
        );
        assert!(generation > 0);
        assert_eq!(velum_client_engine_destroy(handle), VelumControlStatus::Ok);
        assert_eq!(
            unsafe { velum_client_engine_stop(handle, &mut generation) },
            VelumControlStatus::InvalidHandle
        );
    }

    #[test]
    fn engine_proxy_rejects_an_unactivated_pool() {
        let mut handle = 0;
        assert_eq!(
            unsafe { velum_client_engine_create(&mut handle) },
            VelumControlStatus::Ok
        );
        let rules = b"MATCH,PROXY";
        let mut port = 0;
        assert_eq!(
            unsafe { velum_client_engine_proxy_start_v1(handle, 0, slice(rules), &mut port) },
            VelumControlStatus::Internal
        );
        assert_eq!(velum_client_engine_destroy(handle), VelumControlStatus::Ok);
    }

    #[test]
    fn engine_activation_copies_a_resolved_default_node_and_publishes_its_snapshot() {
        let mut id = b"node-one".to_vec();
        let mut alias = b"primary".to_vec();
        let mut relay = b"192.0.2.1:443".to_vec();
        let mut server_name = b"relay.example".to_vec();
        let mut credential = vec![7_u8; 32];
        let node = VelumEngineNodeInput {
            id: slice(&id),
            alias: slice(&alias),
            configuration: crate::VelumClientConfigInput {
                relay_address: slice(&relay),
                server_name: slice(&server_name),
                credential: slice(&credential),
                certificate_pem: slice(&[]),
                connect_timeout_millis: 60_000,
                trust_mode: crate::VELUM_TRUST_SYSTEM,
            },
        };
        let mut handle = 0;
        assert_eq!(
            unsafe { velum_client_engine_create(&mut handle) },
            VelumControlStatus::Ok
        );
        let mut generation = 0;
        assert_eq!(
            unsafe {
                velum_client_engine_activate_v1(handle, &node, 1, slice(&alias), &mut generation)
            },
            VelumControlStatus::Ok
        );

        id.fill(0);
        alias.fill(0);
        relay.fill(0);
        server_name.fill(0);
        credential.fill(0);

        let mut snapshot = VelumEngineNodeSnapshotV1::default();
        assert_eq!(
            unsafe {
                velum_client_engine_node_snapshot_v1(handle, slice(b"node-one"), &mut snapshot)
            },
            VelumControlStatus::Ok
        );
        assert_eq!(snapshot.profile_generation, generation);
        assert_eq!(snapshot.configured, 1);
        assert_eq!(snapshot.is_default, 1);
        assert_eq!(velum_client_engine_destroy(handle), VelumControlStatus::Ok);
    }
}

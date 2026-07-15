use std::{net::SocketAddr, sync::Arc};

use crate::{
    VelumByteSlice, VelumMutableByteSlice, VelumStatus,
    configuration::{copy_bytes, mutable_bytes},
    executor,
    handles::{ClientEntry, handles},
    status_for_client, status_for_runtime,
};

/// Opens a direct reliable stream to an exact target address.
///
/// # Safety
///
/// `target_address` and `out_stream_handle` must be valid pointers for this
/// call. A nonempty target byte slice must be readable UTF-8.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn velum_client_open_stream(
    client_handle: u64,
    target_address: VelumByteSlice,
    out_stream_handle: *mut u64,
) -> VelumStatus {
    if out_stream_handle.is_null() {
        return VelumStatus::InvalidArgument;
    }
    let target = unsafe { copy_bytes(target_address) }.and_then(|value| {
        std::str::from_utf8(&value)
            .ok()
            .and_then(|value| value.parse::<SocketAddr>().ok())
            .ok_or(crate::configuration::ConfigurationInputError::InvalidArgument)
    });
    let target = match target {
        Ok(target) => target,
        Err(error) => return error.into(),
    };
    let entry = match handles().lock() {
        Ok(table) => table.clients.get(&client_handle).cloned(),
        Err(_) => return VelumStatus::Transport,
    };
    let Some(entry) = entry else {
        return VelumStatus::InvalidHandle;
    };
    let stream = match executor().block_on(entry.runtime.open_stream(target)) {
        Ok(stream) => stream,
        Err(error) => return status_for_runtime(error),
    };
    publish_stream_handle(client_handle, &entry, stream, out_stream_handle)
}

fn publish_stream_handle(
    client_handle: u64,
    entry: &Arc<ClientEntry>,
    stream: velum_client_runtime::RuntimeStream,
    out_stream_handle: *mut u64,
) -> VelumStatus {
    let generation = stream.generation();
    let command = match entry.command.lock() {
        Ok(command) => command,
        Err(_) => return VelumStatus::Transport,
    };
    if command.destroyed || !entry.runtime.is_generation_online(generation) {
        return VelumStatus::Transport;
    }
    let handle = match handles().lock() {
        Ok(mut table) => {
            let Some(current) = table.clients.get(&client_handle) else {
                return VelumStatus::InvalidHandle;
            };
            if !Arc::ptr_eq(current, entry) {
                return VelumStatus::InvalidHandle;
            }
            match table.insert_stream(client_handle, stream) {
                Some(handle) => handle,
                None => return VelumStatus::Transport,
            }
        }
        Err(_) => return VelumStatus::Transport,
    };
    unsafe { *out_stream_handle = handle };
    VelumStatus::Ok
}

/// Writes one bounded byte slice to a direct stream.
///
/// # Safety
///
/// A nonempty `input` range must be valid and readable for this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn velum_client_stream_write(
    stream_handle: u64,
    input: VelumByteSlice,
) -> VelumStatus {
    let input = match unsafe { copy_bytes(input) } {
        Ok(input) => input,
        Err(error) => return error.into(),
    };
    let stream = match handles().lock() {
        Ok(table) => table.streams.get(&stream_handle).cloned(),
        Err(_) => return VelumStatus::Transport,
    };
    let Some(stream) = stream else {
        return VelumStatus::InvalidHandle;
    };
    let mut cancellation = stream.cancellation();
    let Ok(mut send) = stream.send.lock() else {
        return VelumStatus::Transport;
    };
    match executor().block_on(async {
        tokio::select! {
            biased;
            _ = cancellation.wait_for(|closed| *closed) => None,
            result = send.write_all(&input) => Some(result),
        }
    }) {
        Some(Ok(())) => VelumStatus::Ok,
        Some(Err(error)) => status_for_client(error),
        None => VelumStatus::Transport,
    }
}

/// Reads one bounded chunk from a direct stream.
///
/// A successful EOF sets `out_read` to zero.
///
/// # Safety
///
/// `output` must identify writable memory and `out_read` must be a valid
/// writable pointer for the duration of this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn velum_client_stream_read(
    stream_handle: u64,
    output: VelumMutableByteSlice,
    out_read: *mut usize,
) -> VelumStatus {
    if out_read.is_null() {
        return VelumStatus::InvalidArgument;
    }
    let output = match unsafe { mutable_bytes(output) } {
        Ok(output) => output,
        Err(status) => return status,
    };
    let stream = match handles().lock() {
        Ok(table) => table.streams.get(&stream_handle).cloned(),
        Err(_) => return VelumStatus::Transport,
    };
    let Some(stream) = stream else {
        return VelumStatus::InvalidHandle;
    };
    let mut cancellation = stream.cancellation();
    let Ok(mut receive) = stream.receive.lock() else {
        return VelumStatus::Transport;
    };
    match executor().block_on(async {
        tokio::select! {
            biased;
            _ = cancellation.wait_for(|closed| *closed) => None,
            result = receive.read(output) => Some(result),
        }
    }) {
        Some(Ok(Some(read))) => {
            unsafe { *out_read = read };
            VelumStatus::Ok
        }
        Some(Ok(None)) => {
            unsafe { *out_read = 0 };
            VelumStatus::Ok
        }
        Some(Err(error)) => status_for_client(error),
        None => VelumStatus::Transport,
    }
}

/// Finishes the local send half while retaining the stream handle for reads.
#[unsafe(no_mangle)]
pub extern "C" fn velum_client_stream_finish(stream_handle: u64) -> VelumStatus {
    let stream = match handles().lock() {
        Ok(table) => table.streams.get(&stream_handle).cloned(),
        Err(_) => return VelumStatus::Transport,
    };
    let Some(stream) = stream else {
        return VelumStatus::InvalidHandle;
    };
    let Ok(mut send) = stream.send.lock() else {
        return VelumStatus::Transport;
    };
    if stream.is_cancelled() {
        return VelumStatus::Transport;
    }
    send.finish()
        .map_or_else(status_for_client, |_| VelumStatus::Ok)
}

/// Invalidates one stream handle and cancels any in-progress local I/O.
#[unsafe(no_mangle)]
pub extern "C" fn velum_client_stream_close(stream_handle: u64) -> VelumStatus {
    let stream = match handles().lock() {
        Ok(mut table) => table.streams.remove(&stream_handle),
        Err(_) => return VelumStatus::Transport,
    };
    let Some(stream) = stream else {
        return VelumStatus::InvalidHandle;
    };
    stream.cancel();
    VelumStatus::Ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stream_api_rejects_unknown_handles() {
        let target = VelumByteSlice {
            pointer: b"192.0.2.10:443".as_ptr(),
            length: b"192.0.2.10:443".len(),
        };
        let mut stream = 0;
        assert_eq!(
            unsafe { velum_client_open_stream(u64::MAX, target, &mut stream) },
            VelumStatus::InvalidHandle
        );
    }
}

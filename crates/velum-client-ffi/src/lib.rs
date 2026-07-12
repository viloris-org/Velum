//! Reviewed native ABI for Flutter's direct Velum client API.
//!
//! The ABI exposes numeric handles only. Write inputs are copied before an
//! asynchronous operation begins; read outputs are written only for the
//! duration of the call. No caller pointer is retained.

use std::{
    collections::BTreeMap,
    io::{BufReader, Cursor},
    net::SocketAddr,
    slice,
    sync::{Arc, Mutex, OnceLock},
    time::Duration,
};

use rustls::pki_types::{CertificateDer, pem::PemObject};
use tokio::runtime::{Builder, Runtime};
use velum_client_api::{Client, ClientConfig, ClientConfigError, ClientError, ClientStream};

/// Stable version for the Flutter native ABI.
pub const ABI_VERSION: u16 = 1;

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

/// Configuration passed by Flutter to create one direct client session.
#[repr(C)]
pub struct VelumClientConfigInput {
    pub relay_address: VelumByteSlice,
    pub server_name: VelumByteSlice,
    pub credential: VelumByteSlice,
    pub certificate_pem: VelumByteSlice,
    pub connect_timeout_millis: u64,
}

#[repr(C)]
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

struct StreamEntry {
    client_handle: u64,
    _client: Arc<Client>,
    stream: Mutex<ClientStream>,
}

#[derive(Default)]
struct HandleTable {
    next_handle: u64,
    clients: BTreeMap<u64, Arc<Client>>,
    streams: BTreeMap<u64, Arc<StreamEntry>>,
}

impl HandleTable {
    fn insert_client(&mut self, client: Client) -> u64 {
        let handle = self.next();
        self.clients.insert(handle, Arc::new(client));
        handle
    }

    fn insert_stream(
        &mut self,
        client_handle: u64,
        client: Arc<Client>,
        stream: ClientStream,
    ) -> u64 {
        let handle = self.next();
        self.streams.insert(
            handle,
            Arc::new(StreamEntry {
                client_handle,
                _client: client,
                stream: Mutex::new(stream),
            }),
        );
        handle
    }

    fn next(&mut self) -> u64 {
        self.next_handle = self
            .next_handle
            .checked_add(1)
            .expect("FFI handle exhausted");
        self.next_handle
    }
}

fn handles() -> &'static Mutex<HandleTable> {
    static HANDLES: OnceLock<Mutex<HandleTable>> = OnceLock::new();
    HANDLES.get_or_init(|| Mutex::new(HandleTable::default()))
}

fn runtime() -> &'static Runtime {
    static RUNTIME: OnceLock<Runtime> = OnceLock::new();
    RUNTIME.get_or_init(|| {
        Builder::new_multi_thread()
            .enable_io()
            .enable_time()
            .build()
            .expect("native client runtime")
    })
}

fn status_for(error: ClientError) -> VelumStatus {
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

fn parse_configuration(
    relay_address: &[u8],
    server_name: &[u8],
    credential: &[u8],
    certificate_pem: &[u8],
    connect_timeout_millis: u64,
) -> Result<ClientConfig, VelumStatus> {
    let relay_address = std::str::from_utf8(relay_address)
        .ok()
        .and_then(|value| value.parse::<SocketAddr>().ok())
        .ok_or(VelumStatus::InvalidArgument)?;
    let server_name = std::str::from_utf8(server_name)
        .map_err(|_| VelumStatus::InvalidArgument)?
        .to_owned();
    let certificates =
        CertificateDer::pem_reader_iter(BufReader::new(Cursor::new(certificate_pem)))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| VelumStatus::Certificate)?;
    ClientConfig::new(
        relay_address,
        server_name,
        credential.to_vec(),
        certificates,
        Duration::from_millis(connect_timeout_millis),
    )
    .map_err(|error| match error {
        ClientConfigError::MissingRootCertificate => VelumStatus::Certificate,
        ClientConfigError::EmptyServerName
        | ClientConfigError::InvalidCredentialLength
        | ClientConfigError::ZeroConnectTimeout => VelumStatus::Configuration,
    })
}

unsafe fn bytes<'a>(value: VelumByteSlice) -> Result<&'a [u8], VelumStatus> {
    if value.length == 0 {
        return Ok(&[]);
    }
    if value.pointer.is_null() {
        return Err(VelumStatus::InvalidArgument);
    }
    // The caller promises a valid immutable byte range for this call only.
    Ok(unsafe { slice::from_raw_parts(value.pointer, value.length) })
}

unsafe fn mutable_bytes<'a>(value: VelumMutableByteSlice) -> Result<&'a mut [u8], VelumStatus> {
    if value.length == 0 || value.pointer.is_null() {
        return Err(VelumStatus::InvalidArgument);
    }
    // The caller promises a valid writable byte range for this call only.
    Ok(unsafe { slice::from_raw_parts_mut(value.pointer, value.length) })
}

/// Returns the ABI version supported by this library.
#[unsafe(no_mangle)]
pub extern "C" fn velum_client_abi_version() -> u16 {
    ABI_VERSION
}

/// Creates and connects a direct client, returning an opaque numeric handle.
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
    if input.is_null() || out_client_handle.is_null() {
        return VelumStatus::InvalidArgument;
    }
    let input = unsafe { &*input };
    let relay_address = match unsafe { bytes(input.relay_address) } {
        Ok(value) => value,
        Err(status) => return status,
    };
    let server_name = match unsafe { bytes(input.server_name) } {
        Ok(value) => value,
        Err(status) => return status,
    };
    let credential = match unsafe { bytes(input.credential) } {
        Ok(value) => value,
        Err(status) => return status,
    };
    let certificate_pem = match unsafe { bytes(input.certificate_pem) } {
        Ok(value) => value,
        Err(status) => return status,
    };
    let configuration = match parse_configuration(
        relay_address,
        server_name,
        credential,
        certificate_pem,
        input.connect_timeout_millis,
    ) {
        Ok(configuration) => configuration,
        Err(status) => return status,
    };
    let client = match runtime().block_on(Client::connect(configuration)) {
        Ok(client) => client,
        Err(error) => return status_for(error),
    };
    let Ok(mut table) = handles().lock() else {
        return VelumStatus::Transport;
    };
    let handle = table.insert_client(client);
    unsafe { *out_client_handle = handle };
    VelumStatus::Ok
}

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
    let target = unsafe { bytes(target_address) }.and_then(|value| {
        std::str::from_utf8(value)
            .ok()
            .and_then(|value| value.parse::<SocketAddr>().ok())
            .ok_or(VelumStatus::InvalidArgument)
    });
    let target = match target {
        Ok(target) => target,
        Err(status) => return status,
    };
    let client = match handles().lock() {
        Ok(table) => table.clients.get(&client_handle).cloned(),
        Err(_) => return VelumStatus::Transport,
    };
    let Some(client) = client else {
        return VelumStatus::InvalidHandle;
    };
    let stream = match runtime().block_on(client.open_stream(target)) {
        Ok(stream) => stream,
        Err(error) => return status_for(error),
    };
    let Ok(mut table) = handles().lock() else {
        return VelumStatus::Transport;
    };
    if !table.clients.contains_key(&client_handle) {
        return VelumStatus::InvalidHandle;
    }
    let handle = table.insert_stream(client_handle, client, stream);
    unsafe { *out_stream_handle = handle };
    VelumStatus::Ok
}

/// Writes one bounded byte slice to a direct stream.
///
/// # Safety
///
/// A nonempty `bytes` range must be valid and readable for this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn velum_client_stream_write(
    stream_handle: u64,
    input: VelumByteSlice,
) -> VelumStatus {
    let input = match unsafe { bytes(input) } {
        Ok(input) => input,
        Err(status) => return status,
    };
    let input = input.to_vec();
    let stream = match handles().lock() {
        Ok(table) => table.streams.get(&stream_handle).cloned(),
        Err(_) => return VelumStatus::Transport,
    };
    let Some(stream) = stream else {
        return VelumStatus::InvalidHandle;
    };
    let Ok(mut stream) = stream.stream.lock() else {
        return VelumStatus::Transport;
    };
    runtime()
        .block_on(stream.write_all(&input))
        .map_or_else(status_for, |_| VelumStatus::Ok)
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
    let Ok(mut stream) = stream.stream.lock() else {
        return VelumStatus::Transport;
    };
    match runtime().block_on(stream.read(output)) {
        Ok(Some(read)) => {
            unsafe { *out_read = read };
            VelumStatus::Ok
        }
        Ok(None) => {
            unsafe { *out_read = 0 };
            VelumStatus::Ok
        }
        Err(error) => status_for(error),
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
    let Ok(mut stream) = stream.stream.lock() else {
        return VelumStatus::Transport;
    };
    stream.finish().map_or_else(status_for, |_| VelumStatus::Ok)
}

/// Invalidates one stream handle and drops both local stream halves.
#[unsafe(no_mangle)]
pub extern "C" fn velum_client_stream_close(stream_handle: u64) -> VelumStatus {
    match handles().lock() {
        Ok(mut table) => {
            if table.streams.remove(&stream_handle).is_some() {
                VelumStatus::Ok
            } else {
                VelumStatus::InvalidHandle
            }
        }
        Err(_) => VelumStatus::Transport,
    }
}

/// Closes a client and invalidates all streams opened from it.
#[unsafe(no_mangle)]
pub extern "C" fn velum_client_close(client_handle: u64) -> VelumStatus {
    let client = match handles().lock() {
        Ok(mut table) => {
            table
                .streams
                .retain(|_, stream| stream.client_handle != client_handle);
            table.clients.remove(&client_handle)
        }
        Err(_) => return VelumStatus::Transport,
    };
    let Some(client) = client else {
        return VelumStatus::InvalidHandle;
    };
    client.close();
    VelumStatus::Ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn configuration_rejects_invalid_or_empty_inputs_without_connecting() {
        assert!(matches!(
            parse_configuration(b"not-an-address", b"relay.example", &[7], b"", 1),
            Err(VelumStatus::InvalidArgument)
        ));
        assert!(matches!(
            parse_configuration(b"192.0.2.1:443", b"relay.example", &[7], b"", 1),
            Err(VelumStatus::Certificate)
        ));
    }

    #[test]
    fn client_close_retires_stream_handles() {
        let mut table = HandleTable {
            next_handle: 3,
            ..Default::default()
        };
        assert_eq!(table.next(), 4);
    }

    #[test]
    fn abi_rejects_invalid_pointers_and_unknown_handles() {
        assert_eq!(
            unsafe { velum_client_connect(std::ptr::null(), std::ptr::null_mut()) },
            VelumStatus::InvalidArgument
        );
        let target = VelumByteSlice {
            pointer: b"192.0.2.10:443".as_ptr(),
            length: b"192.0.2.10:443".len(),
        };
        let mut stream = 0;
        assert_eq!(
            unsafe { velum_client_open_stream(999, target, &mut stream) },
            VelumStatus::InvalidHandle
        );
        assert_eq!(velum_client_close(999), VelumStatus::InvalidHandle);
    }
}

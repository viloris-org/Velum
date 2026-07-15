use std::{
    io::{BufReader, Cursor},
    net::SocketAddr,
    slice,
    time::Duration,
};

use rustls::pki_types::{CertificateDer, pem::PemObject};
use velum_client_runtime::{ClientConfig, ClientConfigError, ClientTrust};

use crate::{
    VELUM_TRUST_CUSTOM_CA, VELUM_TRUST_INSECURE, VELUM_TRUST_SYSTEM, VelumByteSlice,
    VelumClientConfigInput, VelumControlStatus, VelumMutableByteSlice, VelumStatus,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ConfigurationInputError {
    InvalidArgument,
    Configuration,
    Certificate,
}

impl From<ConfigurationInputError> for VelumStatus {
    fn from(error: ConfigurationInputError) -> Self {
        match error {
            ConfigurationInputError::InvalidArgument => Self::InvalidArgument,
            ConfigurationInputError::Configuration => Self::Configuration,
            ConfigurationInputError::Certificate => Self::Certificate,
        }
    }
}

impl From<ConfigurationInputError> for VelumControlStatus {
    fn from(error: ConfigurationInputError) -> Self {
        match error {
            ConfigurationInputError::InvalidArgument => Self::InvalidArgument,
            ConfigurationInputError::Configuration => Self::Configuration,
            ConfigurationInputError::Certificate => Self::Certificate,
        }
    }
}

pub(crate) unsafe fn configuration_from_input(
    input: *const VelumClientConfigInput,
) -> Result<ClientConfig, ConfigurationInputError> {
    if input.is_null() {
        return Err(ConfigurationInputError::InvalidArgument);
    }
    let input = unsafe { &*input };
    let relay_address = unsafe { copy_bytes(input.relay_address) }?;
    let server_name = unsafe { copy_bytes(input.server_name) }?;
    let credential = unsafe { copy_bytes(input.credential) }?;
    let certificate_pem = unsafe { copy_bytes(input.certificate_pem) }?;
    parse_configuration(
        &relay_address,
        &server_name,
        credential,
        certificate_pem,
        input.connect_timeout_millis,
        input.trust_mode,
    )
}

fn parse_configuration(
    relay_address: &[u8],
    server_name: &[u8],
    credential: Vec<u8>,
    certificate_pem: Vec<u8>,
    connect_timeout_millis: u64,
    trust_mode: u32,
) -> Result<ClientConfig, ConfigurationInputError> {
    let relay_address = std::str::from_utf8(relay_address)
        .ok()
        .and_then(|value| value.parse::<SocketAddr>().ok())
        .ok_or(ConfigurationInputError::InvalidArgument)?;
    let server_name = std::str::from_utf8(server_name)
        .map_err(|_| ConfigurationInputError::InvalidArgument)?
        .to_owned();
    let trust = match trust_mode {
        VELUM_TRUST_SYSTEM => ClientTrust::System,
        VELUM_TRUST_CUSTOM_CA => {
            let certificates =
                CertificateDer::pem_reader_iter(BufReader::new(Cursor::new(certificate_pem)))
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|_| ConfigurationInputError::Certificate)?;
            ClientTrust::CustomRoots(certificates)
        }
        VELUM_TRUST_INSECURE => ClientTrust::Insecure,
        _ => return Err(ConfigurationInputError::Configuration),
    };
    ClientConfig::new(
        relay_address,
        server_name,
        credential,
        trust,
        Duration::from_millis(connect_timeout_millis),
    )
    .map_err(|error| match error {
        ClientConfigError::MissingRootCertificate => ConfigurationInputError::Certificate,
        ClientConfigError::EmptyServerName
        | ClientConfigError::InvalidCredentialLength
        | ClientConfigError::ZeroConnectTimeout => ConfigurationInputError::Configuration,
    })
}

pub(crate) unsafe fn copy_bytes(value: VelumByteSlice) -> Result<Vec<u8>, ConfigurationInputError> {
    if value.length == 0 {
        return Ok(Vec::new());
    }
    if value.pointer.is_null() {
        return Err(ConfigurationInputError::InvalidArgument);
    }
    // The caller promises a valid immutable byte range for this call only.
    Ok(unsafe { slice::from_raw_parts(value.pointer, value.length) }.to_vec())
}

pub(crate) unsafe fn mutable_bytes<'a>(
    value: VelumMutableByteSlice,
) -> Result<&'a mut [u8], VelumStatus> {
    if value.length == 0 || value.pointer.is_null() {
        return Err(VelumStatus::InvalidArgument);
    }
    // The caller promises a valid writable byte range for this call only.
    Ok(unsafe { slice::from_raw_parts_mut(value.pointer, value.length) })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn configuration_rejects_invalid_or_empty_inputs_without_connecting() {
        assert!(matches!(
            parse_configuration(
                b"not-an-address",
                b"relay.example",
                vec![7],
                vec![],
                1,
                VELUM_TRUST_SYSTEM,
            ),
            Err(ConfigurationInputError::InvalidArgument)
        ));
        assert!(
            parse_configuration(
                b"192.0.2.1:443",
                b"relay.example",
                vec![7],
                vec![],
                1,
                VELUM_TRUST_SYSTEM,
            )
            .is_ok()
        );
        assert!(
            parse_configuration(
                b"192.0.2.1:443",
                b"relay.example",
                vec![7],
                vec![],
                1,
                VELUM_TRUST_INSECURE,
            )
            .is_ok()
        );
        assert!(matches!(
            parse_configuration(
                b"192.0.2.1:443",
                b"relay.example",
                vec![7],
                vec![],
                1,
                VELUM_TRUST_CUSTOM_CA,
            ),
            Err(ConfigurationInputError::Certificate)
        ));
    }
}

//! Experimental application-owned control record for the QUIC slice.
//!
//! This record is not a Velum wire protocol and must be replaced before Stage 5.

use std::net::SocketAddr;

const MAX_SECRET_BYTES: usize = 128;
const HEADER_BYTES: usize = 4;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OpenRequest {
    pub secret: Vec<u8>,
    pub target: SocketAddr,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ControlError {
    Invalid,
    SecretTooLarge,
}

/// Encodes one request as `secret-length | secret | UTF-8 socket address`.
pub fn encode_open(request: &OpenRequest) -> Result<Vec<u8>, ControlError> {
    if request.secret.is_empty() {
        return Err(ControlError::Invalid);
    }
    let secret_length =
        u16::try_from(request.secret.len()).map_err(|_| ControlError::SecretTooLarge)?;
    if request.secret.len() > MAX_SECRET_BYTES {
        return Err(ControlError::SecretTooLarge);
    }
    let target = request.target.to_string();
    let target_length = u16::try_from(target.len()).map_err(|_| ControlError::Invalid)?;
    let mut encoded = Vec::with_capacity(HEADER_BYTES + request.secret.len() + target.len());
    encoded.extend_from_slice(&secret_length.to_be_bytes());
    encoded.extend_from_slice(&target_length.to_be_bytes());
    encoded.extend_from_slice(&request.secret);
    encoded.extend_from_slice(target.as_bytes());
    Ok(encoded)
}

pub fn decode_open(bytes: &[u8]) -> Result<OpenRequest, ControlError> {
    if bytes.len() < HEADER_BYTES {
        return Err(ControlError::Invalid);
    }
    let secret_length = usize::from(u16::from_be_bytes([bytes[0], bytes[1]]));
    let target_length = usize::from(u16::from_be_bytes([bytes[2], bytes[3]]));
    if secret_length == 0
        || secret_length > MAX_SECRET_BYTES
        || bytes.len() != HEADER_BYTES + secret_length + target_length
    {
        return Err(ControlError::Invalid);
    }
    let secret_end = HEADER_BYTES + secret_length;
    let target = std::str::from_utf8(&bytes[secret_end..])
        .ok()
        .and_then(|value| value.parse().ok())
        .ok_or(ControlError::Invalid)?;
    Ok(OpenRequest {
        secret: bytes[HEADER_BYTES..secret_end].to_vec(),
        target,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_record_round_trips_exact_ip_target() {
        let request = OpenRequest {
            secret: vec![7; 32],
            target: "192.0.2.10:443".parse().expect("target"),
        };
        assert_eq!(
            decode_open(&encode_open(&request).expect("encode")),
            Ok(request)
        );
    }

    #[test]
    fn malformed_and_oversize_records_are_rejected() {
        assert_eq!(decode_open(&[]), Err(ControlError::Invalid));
        assert_eq!(
            encode_open(&OpenRequest {
                secret: vec![0; 129],
                target: "192.0.2.10:443".parse().expect("target"),
            }),
            Err(ControlError::SecretTooLarge)
        );
    }
}

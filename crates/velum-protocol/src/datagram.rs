//! Bounded application envelopes carried by native unreliable datagrams.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};

/// Largest application payload accepted before a carrier's negotiated MTU is
/// applied. The carrier rejects a packet that cannot fit its current path.
pub const MAX_DATAGRAM_PAYLOAD_BYTES: usize = 60 * 1024;
/// The Stage 2 credential is carried only in encrypted client-to-server
/// envelopes. It is bounded independently from the application payload.
pub const MAX_DATAGRAM_CREDENTIAL_BYTES: usize = 128;
pub const MAX_DATAGRAM_ENVELOPE_BYTES: usize =
    MAX_DATAGRAM_PAYLOAD_BYTES + 29 + MAX_DATAGRAM_CREDENTIAL_BYTES;

const CLIENT_TO_SERVER: u8 = 0x01;
const SERVER_TO_CLIENT: u8 = 0x02;
const IPV4: u8 = 0x04;
const IPV6: u8 = 0x06;

/// Opaque, non-zero identifier for one UDP association within a carrier.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DatagramSessionId(u64);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DatagramSessionIdError {
    Zero,
}

impl DatagramSessionId {
    pub const fn new(value: u64) -> Result<Self, DatagramSessionIdError> {
        if value == 0 {
            return Err(DatagramSessionIdError::Zero);
        }
        Ok(Self(value))
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

/// One application datagram, in the direction selected by its envelope type.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Datagram {
    ClientToServer {
        credential: Vec<u8>,
        session_id: DatagramSessionId,
        destination: SocketAddr,
        payload: Vec<u8>,
    },
    ServerToClient {
        session_id: DatagramSessionId,
        source: SocketAddr,
        payload: Vec<u8>,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DatagramEncodeError {
    InvalidCredentialLength {
        minimum: usize,
        maximum: usize,
        actual: usize,
    },
    PayloadTooLarge {
        maximum: usize,
        actual: usize,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DatagramDecodeError {
    TooShort,
    TooLarge {
        maximum: usize,
        actual: usize,
    },
    UnknownDirection(u8),
    InvalidCredentialLength {
        minimum: usize,
        maximum: usize,
        actual: usize,
    },
    InvalidAddressFamily(u8),
    ZeroSessionId,
}

impl Datagram {
    pub fn encode(&self) -> Result<Vec<u8>, DatagramEncodeError> {
        let (direction, credential, session_id, address, payload) = match self {
            Self::ClientToServer {
                credential,
                session_id,
                destination,
                payload,
            } => (
                CLIENT_TO_SERVER,
                Some(credential),
                *session_id,
                *destination,
                payload,
            ),
            Self::ServerToClient {
                session_id,
                source,
                payload,
            } => (SERVER_TO_CLIENT, None, *session_id, *source, payload),
        };
        if let Some(credential) = credential
            && !(1..=MAX_DATAGRAM_CREDENTIAL_BYTES).contains(&credential.len())
        {
            return Err(DatagramEncodeError::InvalidCredentialLength {
                minimum: 1,
                maximum: MAX_DATAGRAM_CREDENTIAL_BYTES,
                actual: credential.len(),
            });
        }
        if payload.len() > MAX_DATAGRAM_PAYLOAD_BYTES {
            return Err(DatagramEncodeError::PayloadTooLarge {
                maximum: MAX_DATAGRAM_PAYLOAD_BYTES,
                actual: payload.len(),
            });
        }
        let mut encoded = Vec::with_capacity(
            28 + credential.map_or(0, |credential| 1 + credential.len()) + payload.len(),
        );
        encoded.push(direction);
        if let Some(credential) = credential {
            encoded.push(u8::try_from(credential.len()).expect("credential length is bounded"));
            encoded.extend_from_slice(credential);
        }
        encoded.extend_from_slice(&session_id.get().to_be_bytes());
        push_address(&mut encoded, address);
        encoded.extend_from_slice(payload);
        Ok(encoded)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, DatagramDecodeError> {
        if bytes.len() > MAX_DATAGRAM_ENVELOPE_BYTES {
            return Err(DatagramDecodeError::TooLarge {
                maximum: MAX_DATAGRAM_ENVELOPE_BYTES,
                actual: bytes.len(),
            });
        }
        let mut reader = Reader::new(bytes);
        let direction = reader.u8()?;
        let credential = if direction == CLIENT_TO_SERVER {
            let length = usize::from(reader.u8()?);
            if !(1..=MAX_DATAGRAM_CREDENTIAL_BYTES).contains(&length) {
                return Err(DatagramDecodeError::InvalidCredentialLength {
                    minimum: 1,
                    maximum: MAX_DATAGRAM_CREDENTIAL_BYTES,
                    actual: length,
                });
            }
            reader.take_slice(length)?.to_vec()
        } else {
            Vec::new()
        };
        let session_id = DatagramSessionId::new(reader.u64()?)
            .map_err(|_| DatagramDecodeError::ZeroSessionId)?;
        let address = reader.address()?;
        let payload = reader.remaining();
        if payload.len() > MAX_DATAGRAM_PAYLOAD_BYTES {
            return Err(DatagramDecodeError::TooLarge {
                maximum: MAX_DATAGRAM_PAYLOAD_BYTES,
                actual: payload.len(),
            });
        }
        match direction {
            CLIENT_TO_SERVER => Ok(Self::ClientToServer {
                credential,
                session_id,
                destination: address,
                payload: payload.to_vec(),
            }),
            SERVER_TO_CLIENT => Ok(Self::ServerToClient {
                session_id,
                source: address,
                payload: payload.to_vec(),
            }),
            unknown => Err(DatagramDecodeError::UnknownDirection(unknown)),
        }
    }
}

fn push_address(bytes: &mut Vec<u8>, address: SocketAddr) {
    match address.ip() {
        IpAddr::V4(ip) => {
            bytes.push(IPV4);
            bytes.extend_from_slice(&ip.octets());
        }
        IpAddr::V6(ip) => {
            bytes.push(IPV6);
            bytes.extend_from_slice(&ip.octets());
        }
    }
    bytes.extend_from_slice(&address.port().to_be_bytes());
}

struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Reader<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn u8(&mut self) -> Result<u8, DatagramDecodeError> {
        let byte = *self
            .bytes
            .get(self.offset)
            .ok_or(DatagramDecodeError::TooShort)?;
        self.offset += 1;
        Ok(byte)
    }

    fn u64(&mut self) -> Result<u64, DatagramDecodeError> {
        Ok(u64::from_be_bytes(self.take()?))
    }

    fn address(&mut self) -> Result<SocketAddr, DatagramDecodeError> {
        let family = self.u8()?;
        let ip = match family {
            IPV4 => IpAddr::V4(Ipv4Addr::from(self.take::<4>()?)),
            IPV6 => IpAddr::V6(Ipv6Addr::from(self.take::<16>()?)),
            unknown => return Err(DatagramDecodeError::InvalidAddressFamily(unknown)),
        };
        let port = u16::from_be_bytes(self.take()?);
        Ok(SocketAddr::new(ip, port))
    }

    fn take<const N: usize>(&mut self) -> Result<[u8; N], DatagramDecodeError> {
        let end = self
            .offset
            .checked_add(N)
            .ok_or(DatagramDecodeError::TooShort)?;
        let bytes = self
            .bytes
            .get(self.offset..end)
            .ok_or(DatagramDecodeError::TooShort)?;
        let mut result = [0; N];
        result.copy_from_slice(bytes);
        self.offset = end;
        Ok(result)
    }

    fn take_slice(&mut self, length: usize) -> Result<&'a [u8], DatagramDecodeError> {
        let end = self
            .offset
            .checked_add(length)
            .ok_or(DatagramDecodeError::TooShort)?;
        let bytes = self
            .bytes
            .get(self.offset..end)
            .ok_or(DatagramDecodeError::TooShort)?;
        self.offset = end;
        Ok(bytes)
    }

    fn remaining(&self) -> &'a [u8] {
        &self.bytes[self.offset..]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session_id() -> DatagramSessionId {
        DatagramSessionId::new(7).expect("non-zero session")
    }

    #[test]
    fn ipv4_and_ipv6_datagrams_round_trip_canonically() {
        let client = Datagram::ClientToServer {
            credential: vec![7; 32],
            session_id: session_id(),
            destination: "192.0.2.10:53".parse().expect("address"),
            payload: vec![1, 2],
        };
        let server = Datagram::ServerToClient {
            session_id: session_id(),
            source: "[2001:db8::1]:443".parse().expect("address"),
            payload: vec![3, 4],
        };
        for datagram in [client, server] {
            assert_eq!(
                Datagram::decode(&datagram.encode().expect("encode")),
                Ok(datagram)
            );
        }
    }

    #[test]
    fn invalid_or_unbounded_envelopes_fail_closed() {
        assert_eq!(DatagramSessionId::new(0), Err(DatagramSessionIdError::Zero));
        assert_eq!(Datagram::decode(&[]), Err(DatagramDecodeError::TooShort));
        assert_eq!(
            Datagram::decode(&[0xff, 0, 0, 0, 0, 0, 0, 0, 1]),
            Err(DatagramDecodeError::TooShort)
        );
        assert_eq!(
            Datagram::decode(&[CLIENT_TO_SERVER, 1, 7, 0, 0, 0, 0, 0, 0, 0, 1, 0xff]),
            Err(DatagramDecodeError::InvalidAddressFamily(0xff))
        );
        assert_eq!(
            Datagram::decode(&[CLIENT_TO_SERVER, 0]),
            Err(DatagramDecodeError::InvalidCredentialLength {
                minimum: 1,
                maximum: MAX_DATAGRAM_CREDENTIAL_BYTES,
                actual: 0,
            })
        );
    }
}

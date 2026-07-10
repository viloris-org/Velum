//! Minimal HTTP CONNECT request parsing for the Stage 2 local adapter.
//!
//! This crate does not open sockets or choose carriers. It translates a local
//! request into an exact IP target that the server admission policy can check.

use std::net::SocketAddr;

pub const MAX_HEADER_BYTES: usize = 8 * 1024;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConnectRequest {
    pub target: SocketAddr,
    pub remaining: Vec<u8>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConnectError {
    Incomplete,
    HeaderTooLarge,
    InvalidRequest,
    UnsupportedMethod,
    UnsupportedVersion,
    HostnamesUnsupported,
}

pub fn parse_request(bytes: &[u8]) -> Result<ConnectRequest, ConnectError> {
    let Some(header_end) = bytes.windows(4).position(|window| window == b"\r\n\r\n") else {
        return if bytes.len() > MAX_HEADER_BYTES {
            Err(ConnectError::HeaderTooLarge)
        } else {
            Err(ConnectError::Incomplete)
        };
    };
    if header_end + 4 > MAX_HEADER_BYTES {
        return Err(ConnectError::HeaderTooLarge);
    }
    let header =
        std::str::from_utf8(&bytes[..header_end]).map_err(|_| ConnectError::InvalidRequest)?;
    let request_line = header.lines().next().ok_or(ConnectError::InvalidRequest)?;
    let mut parts = request_line.split_ascii_whitespace();
    let method = parts.next().ok_or(ConnectError::InvalidRequest)?;
    let authority = parts.next().ok_or(ConnectError::InvalidRequest)?;
    let version = parts.next().ok_or(ConnectError::InvalidRequest)?;
    if parts.next().is_some() {
        return Err(ConnectError::InvalidRequest);
    }
    if method != "CONNECT" {
        return Err(ConnectError::UnsupportedMethod);
    }
    if version != "HTTP/1.1" {
        return Err(ConnectError::UnsupportedVersion);
    }
    let target = authority
        .parse()
        .map_err(|_| ConnectError::HostnamesUnsupported)?;
    Ok(ConnectRequest {
        target,
        remaining: bytes[header_end + 4..].to_vec(),
    })
}

pub const fn connection_established() -> &'static [u8] {
    b"HTTP/1.1 200 Connection Established\r\n\r\n"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_an_ip_connect_request_and_preserves_tunnel_bytes() {
        let request =
            parse_request(b"CONNECT 192.0.2.10:443 HTTP/1.1\r\nHost: ignored\r\n\r\npayload")
                .expect("request");
        assert_eq!(request.target, "192.0.2.10:443".parse().expect("target"));
        assert_eq!(request.remaining, b"payload");
        assert_eq!(
            connection_established(),
            b"HTTP/1.1 200 Connection Established\r\n\r\n"
        );
    }

    #[test]
    fn incomplete_and_hostname_requests_are_rejected() {
        assert_eq!(
            parse_request(b"CONNECT 192.0.2.10:443"),
            Err(ConnectError::Incomplete)
        );
        assert_eq!(
            parse_request(b"CONNECT example.test:443 HTTP/1.1\r\n\r\n"),
            Err(ConnectError::HostnamesUnsupported)
        );
    }
}

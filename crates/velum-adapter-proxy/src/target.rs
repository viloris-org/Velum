use std::{io, net::SocketAddr};

use tokio::net::lookup_host;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ProxyTarget {
    Address(SocketAddr),
    Domain { host: String, port: u16 },
}

impl ProxyTarget {
    pub(crate) fn domain(&self) -> Option<&str> {
        match self {
            Self::Address(_) => None,
            Self::Domain { host, .. } => Some(host),
        }
    }

    pub(crate) fn from_host_port(host: &[u8], port: u16) -> io::Result<Self> {
        if port == 0 {
            return Err(invalid_target("proxy target port must not be zero"));
        }
        let host = std::str::from_utf8(host)
            .map_err(|_| invalid_target("proxy target hostname is not UTF-8"))?;
        validate_hostname(host)?;
        Ok(Self::Domain {
            host: host.to_owned(),
            port,
        })
    }

    pub(crate) fn from_authority(authority: &str) -> io::Result<Self> {
        if let Ok(address) = authority.parse() {
            return Ok(Self::Address(address));
        }
        let (host, port) = authority
            .rsplit_once(':')
            .ok_or_else(|| invalid_target("proxy authority requires a host and port"))?;
        if host.contains(':') {
            return Err(invalid_target("IPv6 proxy authorities must use brackets"));
        }
        let port = port
            .parse::<u16>()
            .map_err(|_| invalid_target("proxy authority contains an invalid port"))?;
        Self::from_host_port(host.as_bytes(), port)
    }

    pub(crate) fn from_http_authority(authority: &str) -> io::Result<Self> {
        if authority.starts_with('[') {
            let end = authority
                .find(']')
                .ok_or_else(|| invalid_target("IPv6 proxy authority is missing a bracket"))?;
            let host = &authority[1..end];
            let ip = host
                .parse()
                .map_err(|_| invalid_target("IPv6 proxy authority is invalid"))?;
            let port = match &authority[end + 1..] {
                "" => 80,
                value => value
                    .strip_prefix(':')
                    .ok_or_else(|| invalid_target("IPv6 proxy authority is invalid"))?
                    .parse()
                    .map_err(|_| invalid_target("proxy authority contains an invalid port"))?,
            };
            return valid_address(SocketAddr::new(ip, port));
        }
        if authority.contains(':') {
            return Self::from_authority(authority);
        }
        if let Ok(ip) = authority.parse() {
            return valid_address(SocketAddr::new(ip, 80));
        }
        Self::from_host_port(authority.as_bytes(), 80)
    }

    pub(crate) async fn resolve(self) -> io::Result<SocketAddr> {
        match self {
            Self::Address(address) => Ok(address),
            Self::Domain { host, port } => lookup_host((host.as_str(), port))
                .await?
                .next()
                .ok_or_else(|| invalid_target("proxy target did not resolve")),
        }
    }
}

fn valid_address(address: SocketAddr) -> io::Result<ProxyTarget> {
    if address.port() == 0 {
        return Err(invalid_target("proxy target port must not be zero"));
    }
    Ok(ProxyTarget::Address(address))
}

fn validate_hostname(host: &str) -> io::Result<()> {
    if host.is_empty()
        || host.len() > 253
        || host.starts_with('.')
        || host.ends_with('.')
        || host
            .split('.')
            .any(|label| label.is_empty() || label.len() > 63)
        || !host
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_'))
    {
        return Err(invalid_target("proxy target hostname is invalid"));
    }
    Ok(())
}

fn invalid_target(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_domain_ipv4_and_bracketed_ipv6_authorities() {
        assert_eq!(
            ProxyTarget::from_authority("example.com:443").expect("domain"),
            ProxyTarget::Domain {
                host: "example.com".into(),
                port: 443,
            }
        );
        assert!(matches!(
            ProxyTarget::from_authority("192.0.2.1:443").expect("IPv4"),
            ProxyTarget::Address(_)
        ));
        assert!(matches!(
            ProxyTarget::from_authority("[2001:db8::1]:443").expect("IPv6"),
            ProxyTarget::Address(_)
        ));
    }

    #[test]
    fn rejects_ambiguous_or_invalid_authorities() {
        for authority in ["example.com", "example.com:0x1bb", "2001:db8::1:443"] {
            assert!(
                ProxyTarget::from_authority(authority).is_err(),
                "{authority}"
            );
        }
    }

    #[test]
    fn http_authorities_default_to_port_eighty() {
        assert_eq!(
            ProxyTarget::from_http_authority("example.com").expect("domain"),
            ProxyTarget::Domain {
                host: "example.com".into(),
                port: 80,
            }
        );
        assert!(matches!(
            ProxyTarget::from_http_authority("[2001:db8::1]").expect("IPv6"),
            ProxyTarget::Address(address) if address.port() == 80
        ));
    }
}

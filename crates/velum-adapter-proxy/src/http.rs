use std::io;

use crate::target::ProxyTarget;

pub(crate) struct ForwardRequest {
    pub(crate) target: ProxyTarget,
    pub(crate) head: Vec<u8>,
}

pub(crate) fn parse_forward_request(head: &[u8]) -> io::Result<ForwardRequest> {
    let text =
        std::str::from_utf8(head).map_err(|_| invalid_request("HTTP request is not UTF-8"))?;
    let text = text
        .strip_suffix("\r\n\r\n")
        .ok_or_else(|| invalid_request("HTTP request header is incomplete"))?;
    let mut lines = text.split("\r\n");
    let request_line = lines
        .next()
        .ok_or_else(|| invalid_request("missing HTTP request line"))?;
    let mut fields = request_line.split(' ');
    let method = fields
        .next()
        .filter(|value| is_token(value))
        .ok_or_else(|| invalid_request("invalid HTTP method"))?;
    let uri = fields
        .next()
        .ok_or_else(|| invalid_request("missing HTTP request target"))?;
    let version = fields
        .next()
        .filter(|value| matches!(*value, "HTTP/1.0" | "HTTP/1.1"))
        .ok_or_else(|| invalid_request("unsupported HTTP version"))?;
    if fields.next().is_some() {
        return Err(invalid_request("invalid HTTP request line"));
    }

    let (authority, origin_form) = split_absolute_http_uri(uri)?;
    let target = ProxyTarget::from_http_authority(authority)?;
    let mut headers = Vec::new();
    let mut connection_tokens = Vec::new();
    for line in lines {
        if line.starts_with([' ', '\t']) {
            return Err(invalid_request("folded HTTP headers are not supported"));
        }
        let (name, value) = line
            .split_once(':')
            .ok_or_else(|| invalid_request("malformed HTTP header"))?;
        if !is_token(name)
            || value
                .bytes()
                .any(|byte| byte.is_ascii_control() && byte != b'\t')
        {
            return Err(invalid_request("malformed HTTP header"));
        }
        if name.eq_ignore_ascii_case("connection") || name.eq_ignore_ascii_case("proxy-connection")
        {
            connection_tokens.extend(
                value
                    .split(',')
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(str::to_ascii_lowercase),
            );
        }
        headers.push((name, value.trim()));
    }

    let mut rewritten = Vec::with_capacity(head.len());
    rewritten.extend_from_slice(method.as_bytes());
    rewritten.push(b' ');
    rewritten.extend_from_slice(origin_form.as_bytes());
    rewritten.push(b' ');
    rewritten.extend_from_slice(version.as_bytes());
    rewritten.extend_from_slice(b"\r\nHost: ");
    rewritten.extend_from_slice(authority.as_bytes());
    rewritten.extend_from_slice(b"\r\nConnection: close\r\n");
    for (name, value) in headers {
        if name.eq_ignore_ascii_case("host")
            || name.eq_ignore_ascii_case("connection")
            || name.eq_ignore_ascii_case("proxy-connection")
            || name.eq_ignore_ascii_case("proxy-authorization")
            || name.eq_ignore_ascii_case("keep-alive")
            || connection_tokens
                .iter()
                .any(|token| name.eq_ignore_ascii_case(token))
        {
            continue;
        }
        rewritten.extend_from_slice(name.as_bytes());
        rewritten.extend_from_slice(b": ");
        rewritten.extend_from_slice(value.as_bytes());
        rewritten.extend_from_slice(b"\r\n");
    }
    rewritten.extend_from_slice(b"\r\n");
    Ok(ForwardRequest {
        target,
        head: rewritten,
    })
}

fn split_absolute_http_uri(uri: &str) -> io::Result<(&str, String)> {
    let scheme = uri
        .get(..7)
        .filter(|value| value.eq_ignore_ascii_case("http://"))
        .ok_or_else(|| invalid_request("HTTP proxy target must use absolute-form http://"))?;
    let remainder = &uri[scheme.len()..];
    let authority_end = remainder.find(['/', '?', '#']).unwrap_or(remainder.len());
    let authority = &remainder[..authority_end];
    if authority.is_empty() || authority.contains('@') {
        return Err(invalid_request("HTTP proxy authority is invalid"));
    }
    let suffix = &remainder[authority_end..];
    if suffix.contains('#') {
        return Err(invalid_request(
            "HTTP request target must not contain a fragment",
        ));
    }
    let origin_form = match suffix.as_bytes().first() {
        None => "/".to_owned(),
        Some(b'?') => format!("/{suffix}"),
        Some(b'/') => suffix.to_owned(),
        _ => return Err(invalid_request("HTTP proxy target is invalid")),
    };
    Ok((authority, origin_form))
}

fn is_token(value: &str) -> bool {
    !value.is_empty()
        && value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric()
                || matches!(
                    byte,
                    b'!' | b'#'
                        | b'$'
                        | b'%'
                        | b'&'
                        | b'\''
                        | b'*'
                        | b'+'
                        | b'-'
                        | b'.'
                        | b'^'
                        | b'_'
                        | b'`'
                        | b'|'
                        | b'~'
                )
        })
}

fn invalid_request(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};

    #[test]
    fn rewrites_absolute_form_and_removes_proxy_hop_headers() {
        let request = parse_forward_request(
            b"GET http://example.com:8080/a?q=1 HTTP/1.1\r\nHost: wrong.example\r\nProxy-Connection: keep-alive, X-Remove\r\nProxy-Authorization: Basic secret\r\nX-Remove: value\r\nX-Keep: yes\r\n\r\n",
        )
        .expect("forward request");
        assert_eq!(
            request.target,
            ProxyTarget::Domain {
                host: "example.com".into(),
                port: 8080,
            }
        );
        let head = String::from_utf8(request.head).expect("UTF-8");
        assert!(head.starts_with("GET /a?q=1 HTTP/1.1\r\nHost: example.com:8080\r\n"));
        assert!(head.contains("X-Keep: yes\r\n"));
        assert!(!head.to_ascii_lowercase().contains("proxy-"));
        assert!(!head.contains("X-Remove"));
    }

    #[test]
    fn parses_default_port_ipv4_and_bracketed_ipv6() {
        let cases = [
            (
                "GET http://192.0.2.1 HTTP/1.1\r\n\r\n",
                SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 0, 2, 1)), 80),
            ),
            (
                "GET http://[2001:db8::1]/ HTTP/1.1\r\n\r\n",
                SocketAddr::new(
                    IpAddr::V6("2001:db8::1".parse::<Ipv6Addr>().expect("IPv6")),
                    80,
                ),
            ),
        ];
        for (head, expected) in cases {
            assert_eq!(
                parse_forward_request(head.as_bytes())
                    .expect("request")
                    .target,
                ProxyTarget::Address(expected)
            );
        }
    }

    #[test]
    fn rejects_https_userinfo_fragments_and_malformed_headers() {
        for request in [
            "GET https://example.com/ HTTP/1.1\r\n\r\n",
            "GET http://user@example.com/ HTTP/1.1\r\n\r\n",
            "GET http://example.com/#fragment HTTP/1.1\r\n\r\n",
            "GET http://example.com/ HTTP/1.1\r\n folded\r\n\r\n",
            "GET http://example.com/ HTTP/1.1\r\nX-Bad: value\x01\r\n\r\n",
            "GET http://example.com/ HTTP/2\r\n\r\n",
        ] {
            assert!(
                parse_forward_request(request.as_bytes()).is_err(),
                "{request:?}"
            );
        }
    }
}

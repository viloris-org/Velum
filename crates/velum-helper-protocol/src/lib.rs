//! Bounded control protocol shared by unprivileged clients and privileged traffic hosts.
//!
//! The protocol deliberately cannot carry filesystem paths, shell commands,
//! credentials, certificates, or traffic payloads. Peer authorization and
//! platform handle transfer belong to the platform transport, not this codec.

use std::{collections::BTreeSet, net::IpAddr};

use serde::{Deserialize, Serialize, de::DeserializeOwned};

/// Current helper protocol version.
pub const PROTOCOL_VERSION: u16 = 1;
/// Maximum JSON body size. The four-byte frame header is not included.
pub const MAX_FRAME_LENGTH: usize = 64 * 1024;
const FRAME_HEADER_LENGTH: usize = 4;

/// Stable feature names negotiated by the controller and traffic host.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Capability {
    Tun,
    Ipv6,
    RouteTransaction,
    DnsTransaction,
    SocketProtection,
    CrashRecovery,
}

/// One IP network installed on a TUN interface.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct IpNetwork {
    pub address: IpAddr,
    pub prefix_length: u8,
}

/// Complete, bounded network configuration accepted by `start`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StartParameters {
    pub mtu: u16,
    pub ipv4: Option<IpNetwork>,
    pub ipv6: Option<IpNetwork>,
    pub dns: Vec<IpAddr>,
}

/// A parameter object for commands which intentionally accept no arguments.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EmptyParameters {}

/// The only operations a privileged traffic host may execute.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "command", content = "parameters", rename_all = "snake_case")]
pub enum Command {
    Hello(EmptyParameters),
    Status(EmptyParameters),
    Start(StartParameters),
    Stop(EmptyParameters),
    Recover(EmptyParameters),
}

/// A controller request. Request IDs make retries idempotent at the host boundary.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub version: u16,
    pub request_id: u64,
    pub profile_generation: u64,
    pub capabilities: BTreeSet<Capability>,
    #[serde(flatten)]
    pub command: Command,
}

/// Lifecycle state reported without platform error strings or sensitive data.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HostState {
    Recovering,
    Stopped,
    Starting,
    Running,
    Stopping,
    Failed,
}

/// Stable, payload-free failure categories returned by a traffic host.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    Unauthorized,
    UnsupportedVersion,
    UnsupportedCapability,
    InvalidConfiguration,
    GenerationConflict,
    Busy,
    Platform,
    RecoveryRequired,
}

/// Successful command-specific response data.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Success {
    Hello,
    Status { state: HostState },
    Started,
    Stopped,
    Recovered,
}

/// Result of one helper command. Errors intentionally contain no free-form message.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "result", content = "value", rename_all = "snake_case")]
pub enum CommandResult {
    Ok(Success),
    Err(ErrorCode),
}

/// A traffic-host response correlated to exactly one request.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Response {
    pub version: u16,
    pub request_id: u64,
    pub profile_generation: u64,
    pub capabilities: BTreeSet<Capability>,
    pub response: CommandResult,
}

/// Framing or JSON validation failed before a command reached the host.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CodecError {
    EmptyFrame,
    FrameTooLarge,
    InvalidJson,
    UnsupportedVersion,
}

/// Encodes one request as a four-byte big-endian length followed by JSON.
pub fn encode_request(request: &Request) -> Result<Vec<u8>, CodecError> {
    encode(request)
}

/// Encodes one response as a four-byte big-endian length followed by JSON.
pub fn encode_response(response: &Response) -> Result<Vec<u8>, CodecError> {
    encode(response)
}

/// Decodes and strictly validates one unframed request body.
pub fn decode_request(body: &[u8]) -> Result<Request, CodecError> {
    decode(body)
}

/// Decodes and strictly validates one unframed response body.
pub fn decode_response(body: &[u8]) -> Result<Response, CodecError> {
    decode(body)
}

fn encode<T: Serialize>(value: &T) -> Result<Vec<u8>, CodecError> {
    let body = serde_json::to_vec(value).map_err(|_| CodecError::InvalidJson)?;
    if body.is_empty() {
        return Err(CodecError::EmptyFrame);
    }
    if body.len() > MAX_FRAME_LENGTH {
        return Err(CodecError::FrameTooLarge);
    }
    let length = u32::try_from(body.len()).map_err(|_| CodecError::FrameTooLarge)?;
    let mut frame = Vec::with_capacity(FRAME_HEADER_LENGTH + body.len());
    frame.extend_from_slice(&length.to_be_bytes());
    frame.extend_from_slice(&body);
    Ok(frame)
}

trait Versioned {
    fn version(&self) -> u16;
}

impl Versioned for Request {
    fn version(&self) -> u16 {
        self.version
    }
}

impl Versioned for Response {
    fn version(&self) -> u16 {
        self.version
    }
}

fn decode<T: DeserializeOwned + Versioned>(body: &[u8]) -> Result<T, CodecError> {
    if body.is_empty() {
        return Err(CodecError::EmptyFrame);
    }
    if body.len() > MAX_FRAME_LENGTH {
        return Err(CodecError::FrameTooLarge);
    }
    let decoded: T = serde_json::from_slice(body).map_err(|_| CodecError::InvalidJson)?;
    if decoded.version() != PROTOCOL_VERSION {
        return Err(CodecError::UnsupportedVersion);
    }
    Ok(decoded)
}

/// Incremental request decoder supporting fragmented and coalesced stream reads.
#[derive(Debug, Default)]
pub struct RequestDecoder {
    buffer: Vec<u8>,
    expected_body_length: Option<usize>,
}

impl RequestDecoder {
    pub fn new() -> Self {
        Self::default()
    }

    /// Appends bytes and returns every complete request now available.
    pub fn push(&mut self, mut bytes: &[u8]) -> Result<Vec<Request>, CodecError> {
        let mut requests = Vec::new();
        loop {
            let body_length = match self.expected_body_length {
                Some(length) => length,
                None => {
                    let required = FRAME_HEADER_LENGTH - self.buffer.len();
                    let consumed = required.min(bytes.len());
                    self.buffer.extend_from_slice(&bytes[..consumed]);
                    bytes = &bytes[consumed..];
                    if self.buffer.len() < FRAME_HEADER_LENGTH {
                        break;
                    }
                    let header: [u8; FRAME_HEADER_LENGTH] = self.buffer[..FRAME_HEADER_LENGTH]
                        .try_into()
                        .expect("frame header length checked");
                    self.buffer.clear();
                    let length = u32::from_be_bytes(header) as usize;
                    if length == 0 {
                        self.clear();
                        return Err(CodecError::EmptyFrame);
                    }
                    if length > MAX_FRAME_LENGTH {
                        self.clear();
                        return Err(CodecError::FrameTooLarge);
                    }
                    self.expected_body_length = Some(length);
                    length
                }
            };
            let required = body_length - self.buffer.len();
            let consumed = required.min(bytes.len());
            self.buffer.extend_from_slice(&bytes[..consumed]);
            bytes = &bytes[consumed..];
            if self.buffer.len() < body_length {
                break;
            }
            let request = decode_request(&self.buffer);
            self.buffer.clear();
            self.expected_body_length = None;
            match request {
                Ok(request) => requests.push(request),
                Err(error) => {
                    self.clear();
                    return Err(error);
                }
            }
            if bytes.is_empty() {
                break;
            }
        }
        Ok(requests)
    }

    /// Discards a failed or disconnected peer's partial frame.
    pub fn clear(&mut self) {
        self.buffer.clear();
        self.expected_body_length = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(command: Command) -> Request {
        Request {
            version: PROTOCOL_VERSION,
            request_id: 17,
            profile_generation: 8,
            capabilities: BTreeSet::from([
                Capability::Tun,
                Capability::RouteTransaction,
                Capability::CrashRecovery,
            ]),
            command,
        }
    }

    #[test]
    fn incremental_decoder_accepts_fragmented_and_coalesced_frames() {
        let hello = encode_request(&request(Command::Hello(EmptyParameters {}))).expect("hello");
        let status = encode_request(&request(Command::Status(EmptyParameters {}))).expect("status");
        let split = hello.len() / 2;
        let mut decoder = RequestDecoder::new();

        assert!(decoder.push(&hello[..split]).expect("partial").is_empty());
        let mut remainder = hello[split..].to_vec();
        remainder.extend_from_slice(&status);
        let decoded = decoder.push(&remainder).expect("complete frames");

        assert_eq!(decoded.len(), 2);
        assert!(matches!(decoded[0].command, Command::Hello(_)));
        assert!(matches!(decoded[1].command, Command::Status(_)));
    }

    #[test]
    fn every_whitelisted_command_round_trips() {
        let commands = [
            Command::Hello(EmptyParameters {}),
            Command::Status(EmptyParameters {}),
            Command::Start(StartParameters {
                mtu: 1500,
                ipv4: Some(IpNetwork {
                    address: "172.19.0.1".parse().expect("IPv4"),
                    prefix_length: 30,
                }),
                ipv6: Some(IpNetwork {
                    address: "fd00:19::1".parse().expect("IPv6"),
                    prefix_length: 126,
                }),
                dns: vec!["1.1.1.1".parse().expect("DNS")],
            }),
            Command::Stop(EmptyParameters {}),
            Command::Recover(EmptyParameters {}),
        ];

        for command in commands {
            let expected = request(command);
            let frame = encode_request(&expected).expect("encode");
            let decoded = decode_request(&frame[FRAME_HEADER_LENGTH..]).expect("decode");
            assert_eq!(decoded, expected);
        }
    }

    #[test]
    fn decoder_rejects_oversized_frames_before_reading_the_body() {
        let mut decoder = RequestDecoder::new();
        let header = u32::try_from(MAX_FRAME_LENGTH + 1)
            .expect("bounded constant")
            .to_be_bytes();
        assert_eq!(decoder.push(&header), Err(CodecError::FrameTooLarge));
    }

    #[test]
    fn oversized_chunk_does_not_bypass_the_declared_frame_length() {
        let mut decoder = RequestDecoder::new();
        let mut bytes = 1_u32.to_be_bytes().to_vec();
        bytes.extend(std::iter::repeat_n(b'{', MAX_FRAME_LENGTH + 1));

        assert_eq!(decoder.push(&bytes), Err(CodecError::InvalidJson));
        assert!(decoder.buffer.is_empty());
        assert_eq!(decoder.expected_body_length, None);
    }

    #[test]
    fn strict_json_rejects_unknown_commands_and_fields() {
        let unknown_command = br#"{"version":1,"request_id":1,"profile_generation":1,"capabilities":[],"command":"execute","parameters":{}}"#;
        assert_eq!(
            decode_request(unknown_command),
            Err(CodecError::InvalidJson)
        );

        let unknown_field = br#"{"version":1,"request_id":1,"profile_generation":1,"capabilities":[],"command":"hello","parameters":{},"path":"/tmp/value"}"#;
        assert_eq!(decode_request(unknown_field), Err(CodecError::InvalidJson));

        let unknown_parameter = br#"{"version":1,"request_id":1,"profile_generation":1,"capabilities":[],"command":"stop","parameters":{"shell":"id"}}"#;
        assert_eq!(
            decode_request(unknown_parameter),
            Err(CodecError::InvalidJson)
        );
    }

    #[test]
    fn decoder_rejects_other_protocol_versions() {
        let body = br#"{"version":2,"request_id":1,"profile_generation":1,"capabilities":[],"command":"hello","parameters":{}}"#;
        assert_eq!(decode_request(body), Err(CodecError::UnsupportedVersion));
    }

    #[test]
    fn response_errors_are_bounded_categories_without_messages() {
        let response = Response {
            version: PROTOCOL_VERSION,
            request_id: 17,
            profile_generation: 8,
            capabilities: BTreeSet::from([Capability::Tun]),
            response: CommandResult::Err(ErrorCode::Unauthorized),
        };
        let frame = encode_response(&response).expect("response");
        assert_eq!(decode_response(&frame[FRAME_HEADER_LENGTH..]), Ok(response));
    }
}

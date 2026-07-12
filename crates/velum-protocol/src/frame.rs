//! Bounded canonical v0 frame encoding.
//!
//! Frames are only valid inside an authenticated, encrypted carrier. The
//! header intentionally has no Velum magic value so it cannot act as a
//! pre-authentication marker.

use crate::{
    AttachmentId, Capabilities, CapabilityAdvertisement, Epoch, FlowId, HandshakeNonce,
    NegotiatedParameters, NegotiationOffer, ProtocolErrorCode, Sequence, SessionId, VersionRange,
};

pub const FRAME_HEADER_BYTES: usize = 4;
pub const MAX_FRAME_PAYLOAD: usize = u16::MAX as usize;

const NEGOTIATION_OFFER: u8 = 0x01;
const NEGOTIATION_ACCEPT: u8 = 0x02;
const ATTACH: u8 = 0x03;
const ATTACH_ACCEPTED: u8 = 0x04;
const STREAM_OPEN: u8 = 0x10;
const STREAM_DATA: u8 = 0x11;
const STREAM_ACK: u8 = 0x12;
const STREAM_FINISH: u8 = 0x13;
const STREAM_RESET: u8 = 0x14;
const CONNECTION_CLOSE: u8 = 0x20;
const OPTIONAL_FRAME_BIT: u8 = 0x80;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Frame {
    NegotiationOffer(NegotiationOffer),
    NegotiationAccept(NegotiatedParameters),
    Attach {
        session_id: SessionId,
        epoch: Epoch,
        attachment_id: AttachmentId,
        parameters: NegotiatedParameters,
        proof: [u8; 32],
    },
    AttachAccepted {
        session_id: SessionId,
        epoch: Epoch,
        attachment_id: AttachmentId,
    },
    StreamOpen {
        flow_id: FlowId,
    },
    StreamData {
        flow_id: FlowId,
        epoch: Epoch,
        sequence: Sequence,
        bytes: Vec<u8>,
    },
    StreamAcknowledgement {
        flow_id: FlowId,
        epoch: Epoch,
        through: Sequence,
    },
    StreamFinish {
        flow_id: FlowId,
        epoch: Epoch,
        final_next_sequence: Sequence,
    },
    StreamReset {
        flow_id: FlowId,
        error: ProtocolErrorCode,
    },
    ConnectionClose {
        error: ProtocolErrorCode,
    },
    /// An unrecognized optional frame. Recipients can ignore it after bounded
    /// parsing, but must not reinterpret it as a known frame.
    UnknownOptional {
        frame_type: u8,
        payload: Vec<u8>,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FrameEncodeError {
    PayloadTooLarge { maximum: usize, actual: usize },
    EmptyStreamData,
    InvalidOptionalFrameType(u8),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FrameDecodeError {
    TruncatedHeader,
    LengthMismatch { advertised: usize, actual: usize },
    PayloadTooLarge { maximum: usize, actual: usize },
    UnknownMandatoryFrame(u8),
    EmptyStreamData,
    InvalidPayload,
}

impl Frame {
    pub fn encode(&self) -> Result<Vec<u8>, FrameEncodeError> {
        let (frame_type, payload) = self.encode_payload()?;
        if payload.len() > MAX_FRAME_PAYLOAD {
            return Err(FrameEncodeError::PayloadTooLarge {
                maximum: MAX_FRAME_PAYLOAD,
                actual: payload.len(),
            });
        }
        let payload_length =
            u16::try_from(payload.len()).map_err(|_| FrameEncodeError::PayloadTooLarge {
                maximum: MAX_FRAME_PAYLOAD,
                actual: payload.len(),
            })?;
        let mut encoded = Vec::with_capacity(FRAME_HEADER_BYTES + payload.len());
        encoded.push(frame_type);
        encoded.push(0);
        encoded.extend_from_slice(&payload_length.to_be_bytes());
        encoded.extend_from_slice(&payload);
        Ok(encoded)
    }

    pub fn decode_exact(bytes: &[u8], maximum_payload: usize) -> Result<Self, FrameDecodeError> {
        if bytes.len() < FRAME_HEADER_BYTES {
            return Err(FrameDecodeError::TruncatedHeader);
        }
        let advertised = usize::from(u16::from_be_bytes([bytes[2], bytes[3]]));
        if advertised > maximum_payload {
            return Err(FrameDecodeError::PayloadTooLarge {
                maximum: maximum_payload,
                actual: advertised,
            });
        }
        let actual = bytes.len() - FRAME_HEADER_BYTES;
        if actual != advertised {
            return Err(FrameDecodeError::LengthMismatch { advertised, actual });
        }
        if bytes[1] != 0 {
            return Err(FrameDecodeError::InvalidPayload);
        }
        Self::decode_payload(bytes[0], &bytes[FRAME_HEADER_BYTES..])
    }

    fn encode_payload(&self) -> Result<(u8, Vec<u8>), FrameEncodeError> {
        let mut payload = Vec::new();
        let frame_type = match self {
            Self::NegotiationOffer(offer) => {
                push_version_range(&mut payload, offer.versions);
                push_capabilities(&mut payload, offer.capabilities.required);
                push_capabilities(&mut payload, offer.capabilities.optional);
                payload.extend_from_slice(&offer.client_nonce.bytes());
                NEGOTIATION_OFFER
            }
            Self::NegotiationAccept(parameters) => {
                payload.extend_from_slice(&parameters.canonical_bytes());
                NEGOTIATION_ACCEPT
            }
            Self::Attach {
                session_id,
                epoch,
                attachment_id,
                parameters,
                proof,
            } => {
                payload.extend_from_slice(&session_id.0);
                push_epoch(&mut payload, *epoch);
                payload.extend_from_slice(&attachment_id.bytes());
                payload.extend_from_slice(&parameters.canonical_bytes());
                payload.extend_from_slice(proof);
                ATTACH
            }
            Self::AttachAccepted {
                session_id,
                epoch,
                attachment_id,
            } => {
                payload.extend_from_slice(&session_id.0);
                push_epoch(&mut payload, *epoch);
                payload.extend_from_slice(&attachment_id.bytes());
                ATTACH_ACCEPTED
            }
            Self::StreamOpen { flow_id } => {
                push_flow_id(&mut payload, *flow_id);
                STREAM_OPEN
            }
            Self::StreamData {
                flow_id,
                epoch,
                sequence,
                bytes,
            } => {
                if bytes.is_empty() {
                    return Err(FrameEncodeError::EmptyStreamData);
                }
                push_flow_id(&mut payload, *flow_id);
                push_epoch(&mut payload, *epoch);
                push_sequence(&mut payload, *sequence);
                payload.extend_from_slice(bytes);
                STREAM_DATA
            }
            Self::StreamAcknowledgement {
                flow_id,
                epoch,
                through,
            } => {
                push_flow_id(&mut payload, *flow_id);
                push_epoch(&mut payload, *epoch);
                push_sequence(&mut payload, *through);
                STREAM_ACK
            }
            Self::StreamFinish {
                flow_id,
                epoch,
                final_next_sequence,
            } => {
                push_flow_id(&mut payload, *flow_id);
                push_epoch(&mut payload, *epoch);
                push_sequence(&mut payload, *final_next_sequence);
                STREAM_FINISH
            }
            Self::StreamReset { flow_id, error } => {
                push_flow_id(&mut payload, *flow_id);
                push_error(&mut payload, *error);
                STREAM_RESET
            }
            Self::ConnectionClose { error } => {
                push_error(&mut payload, *error);
                CONNECTION_CLOSE
            }
            Self::UnknownOptional {
                frame_type,
                payload: unknown_payload,
            } => {
                if frame_type & OPTIONAL_FRAME_BIT == 0 {
                    return Err(FrameEncodeError::InvalidOptionalFrameType(*frame_type));
                }
                payload.extend_from_slice(unknown_payload);
                *frame_type
            }
        };
        Ok((frame_type, payload))
    }

    fn decode_payload(frame_type: u8, payload: &[u8]) -> Result<Self, FrameDecodeError> {
        let mut reader = Reader::new(payload);
        let frame = match frame_type {
            NEGOTIATION_OFFER => {
                let versions = reader.version_range()?;
                let required = reader.capabilities()?;
                let optional = reader.capabilities()?;
                let client_nonce = reader.nonce()?;
                let capabilities = CapabilityAdvertisement::new(required, optional)
                    .map_err(|_| FrameDecodeError::InvalidPayload)?;
                Self::NegotiationOffer(
                    NegotiationOffer::new(versions, capabilities, client_nonce)
                        .map_err(|_| FrameDecodeError::InvalidPayload)?,
                )
            }
            NEGOTIATION_ACCEPT => Self::NegotiationAccept(reader.parameters()?),
            ATTACH => Self::Attach {
                session_id: SessionId(reader.array()?),
                epoch: Epoch(reader.u64()?),
                attachment_id: reader.attachment_id()?,
                parameters: reader.parameters()?,
                proof: reader.array()?,
            },
            ATTACH_ACCEPTED => Self::AttachAccepted {
                session_id: SessionId(reader.array()?),
                epoch: Epoch(reader.u64()?),
                attachment_id: reader.attachment_id()?,
            },
            STREAM_OPEN => Self::StreamOpen {
                flow_id: FlowId(reader.u64()?),
            },
            STREAM_DATA => {
                let flow_id = FlowId(reader.u64()?);
                let epoch = Epoch(reader.u64()?);
                let sequence = Sequence(reader.u64()?);
                let bytes = reader.take_remaining().to_vec();
                if bytes.is_empty() {
                    return Err(FrameDecodeError::EmptyStreamData);
                }
                Self::StreamData {
                    flow_id,
                    epoch,
                    sequence,
                    bytes,
                }
            }
            STREAM_ACK => Self::StreamAcknowledgement {
                flow_id: FlowId(reader.u64()?),
                epoch: Epoch(reader.u64()?),
                through: Sequence(reader.u64()?),
            },
            STREAM_FINISH => Self::StreamFinish {
                flow_id: FlowId(reader.u64()?),
                epoch: Epoch(reader.u64()?),
                final_next_sequence: Sequence(reader.u64()?),
            },
            STREAM_RESET => Self::StreamReset {
                flow_id: FlowId(reader.u64()?),
                error: reader.error()?,
            },
            CONNECTION_CLOSE => Self::ConnectionClose {
                error: reader.error()?,
            },
            unknown if unknown & OPTIONAL_FRAME_BIT != 0 => Self::UnknownOptional {
                frame_type: unknown,
                payload: reader.take_remaining().to_vec(),
            },
            unknown => return Err(FrameDecodeError::UnknownMandatoryFrame(unknown)),
        };
        if reader.remaining().is_empty() {
            Ok(frame)
        } else {
            Err(FrameDecodeError::InvalidPayload)
        }
    }
}

/// Incremental decoder that never retains more than one declared frame.
#[derive(Debug)]
pub struct FrameDecoder {
    maximum_payload: usize,
    buffered: Vec<u8>,
}

impl FrameDecoder {
    pub fn new(maximum_payload: usize) -> Result<Self, FrameDecodeError> {
        if maximum_payload > MAX_FRAME_PAYLOAD {
            return Err(FrameDecodeError::PayloadTooLarge {
                maximum: MAX_FRAME_PAYLOAD,
                actual: maximum_payload,
            });
        }
        Ok(Self {
            maximum_payload,
            buffered: Vec::new(),
        })
    }

    pub fn push(&mut self, bytes: &[u8]) -> Result<Vec<Frame>, FrameDecodeError> {
        let mut frames = Vec::new();
        for byte in bytes {
            self.buffered.push(*byte);
            loop {
                if self.buffered.len() < FRAME_HEADER_BYTES {
                    break;
                }
                let advertised =
                    usize::from(u16::from_be_bytes([self.buffered[2], self.buffered[3]]));
                if advertised > self.maximum_payload {
                    self.buffered.clear();
                    return Err(FrameDecodeError::PayloadTooLarge {
                        maximum: self.maximum_payload,
                        actual: advertised,
                    });
                }
                let total = FRAME_HEADER_BYTES + advertised;
                if self.buffered.len() < total {
                    break;
                }
                let frame = Frame::decode_exact(&self.buffered[..total], self.maximum_payload)?;
                self.buffered.drain(..total);
                frames.push(frame);
            }
        }
        Ok(frames)
    }

    pub fn buffered_len(&self) -> usize {
        self.buffered.len()
    }
}

struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Reader<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], FrameDecodeError> {
        let end = self
            .offset
            .checked_add(N)
            .ok_or(FrameDecodeError::InvalidPayload)?;
        let slice = self
            .bytes
            .get(self.offset..end)
            .ok_or(FrameDecodeError::InvalidPayload)?;
        let mut result = [0; N];
        result.copy_from_slice(slice);
        self.offset = end;
        Ok(result)
    }

    fn u16(&mut self) -> Result<u16, FrameDecodeError> {
        Ok(u16::from_be_bytes(self.array()?))
    }

    fn u64(&mut self) -> Result<u64, FrameDecodeError> {
        Ok(u64::from_be_bytes(self.array()?))
    }

    fn version_range(&mut self) -> Result<VersionRange, FrameDecodeError> {
        VersionRange::new(self.u16()?, self.u16()?).map_err(|_| FrameDecodeError::InvalidPayload)
    }

    fn capabilities(&mut self) -> Result<Capabilities, FrameDecodeError> {
        Ok(Capabilities(self.u64()?))
    }

    fn nonce(&mut self) -> Result<HandshakeNonce, FrameDecodeError> {
        HandshakeNonce::new(self.array()?).map_err(|_| FrameDecodeError::InvalidPayload)
    }

    fn attachment_id(&mut self) -> Result<AttachmentId, FrameDecodeError> {
        AttachmentId::new(self.array()?).map_err(|_| FrameDecodeError::InvalidPayload)
    }

    fn parameters(&mut self) -> Result<NegotiatedParameters, FrameDecodeError> {
        Ok(NegotiatedParameters {
            version: self.u16()?,
            capabilities: self.capabilities()?,
            client_nonce: self.nonce()?,
            server_nonce: self.nonce()?,
        })
    }

    fn error(&mut self) -> Result<ProtocolErrorCode, FrameDecodeError> {
        Ok(ProtocolErrorCode::from_u16(self.u16()?))
    }

    fn remaining(&self) -> &'a [u8] {
        &self.bytes[self.offset..]
    }

    fn take_remaining(&mut self) -> &'a [u8] {
        let remaining = self.remaining();
        self.offset = self.bytes.len();
        remaining
    }
}

fn push_version_range(bytes: &mut Vec<u8>, range: VersionRange) {
    bytes.extend_from_slice(&range.minimum.to_be_bytes());
    bytes.extend_from_slice(&range.maximum.to_be_bytes());
}

fn push_capabilities(bytes: &mut Vec<u8>, capabilities: Capabilities) {
    bytes.extend_from_slice(&capabilities.0.to_be_bytes());
}

fn push_u64(bytes: &mut Vec<u8>, value: u64) {
    bytes.extend_from_slice(&value.to_be_bytes());
}

fn push_flow_id(bytes: &mut Vec<u8>, flow_id: FlowId) {
    push_u64(bytes, flow_id.0);
}

fn push_epoch(bytes: &mut Vec<u8>, epoch: Epoch) {
    push_u64(bytes, epoch.0);
}

fn push_sequence(bytes: &mut Vec<u8>, sequence: Sequence) {
    push_u64(bytes, sequence.0);
}

fn push_error(bytes: &mut Vec<u8>, error: ProtocolErrorCode) {
    bytes.extend_from_slice(&error.as_u16().to_be_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        AttachmentId, CapabilityAdvertisement, HandshakeNonce, NegotiationOffer, VersionRange,
    };

    fn nonce(byte: u8) -> HandshakeNonce {
        HandshakeNonce::new([byte; 16]).expect("non-zero nonce")
    }

    fn parameters() -> NegotiatedParameters {
        NegotiationOffer::new(
            VersionRange::new(0, 0).expect("range"),
            CapabilityAdvertisement::new(Capabilities::RELIABLE_STREAM, Capabilities(0))
                .expect("capabilities"),
            nonce(1),
        )
        .expect("offer")
        .negotiate(
            VersionRange::new(0, 0).expect("range"),
            Capabilities::RELIABLE_STREAM,
            nonce(2),
        )
        .expect("parameters")
    }

    #[test]
    fn all_known_frames_round_trip_canonically() {
        let frames = [
            Frame::NegotiationOffer(
                NegotiationOffer::new(
                    VersionRange::new(0, 0).expect("range"),
                    CapabilityAdvertisement::new(Capabilities::RELIABLE_STREAM, Capabilities(0))
                        .expect("capabilities"),
                    nonce(1),
                )
                .expect("offer"),
            ),
            Frame::NegotiationAccept(parameters()),
            Frame::Attach {
                session_id: SessionId([3; 16]),
                epoch: Epoch(5),
                attachment_id: AttachmentId::new([4; 16]).expect("attachment id"),
                parameters: parameters(),
                proof: [6; 32],
            },
            Frame::AttachAccepted {
                session_id: SessionId([3; 16]),
                epoch: Epoch(5),
                attachment_id: AttachmentId::new([4; 16]).expect("attachment id"),
            },
            Frame::StreamOpen { flow_id: FlowId(7) },
            Frame::StreamData {
                flow_id: FlowId(7),
                epoch: Epoch(5),
                sequence: Sequence(8),
                bytes: vec![9, 10],
            },
            Frame::StreamAcknowledgement {
                flow_id: FlowId(7),
                epoch: Epoch(5),
                through: Sequence(8),
            },
            Frame::StreamFinish {
                flow_id: FlowId(7),
                epoch: Epoch(5),
                final_next_sequence: Sequence(9),
            },
            Frame::StreamReset {
                flow_id: FlowId(7),
                error: ProtocolErrorCode::FlowState,
            },
            Frame::ConnectionClose {
                error: ProtocolErrorCode::ResourceLimit,
            },
        ];

        for frame in frames {
            let encoded = frame.encode().expect("encode");
            assert_eq!(Frame::decode_exact(&encoded, MAX_FRAME_PAYLOAD), Ok(frame));
        }
    }

    #[test]
    fn decoder_handles_split_and_coalesced_frames_without_unbounded_buffering() {
        let first = Frame::StreamOpen { flow_id: FlowId(1) }
            .encode()
            .expect("encode");
        let second = Frame::ConnectionClose {
            error: ProtocolErrorCode::ProtocolViolation,
        }
        .encode()
        .expect("encode");
        let mut decoder = FrameDecoder::new(64).expect("decoder");
        let mut encoded = first;
        encoded.extend_from_slice(&second);
        let mut decoded = Vec::new();
        for byte in encoded {
            decoded.extend(decoder.push(&[byte]).expect("incremental decode"));
        }
        assert_eq!(
            decoded,
            vec![
                Frame::StreamOpen { flow_id: FlowId(1) },
                Frame::ConnectionClose {
                    error: ProtocolErrorCode::ProtocolViolation,
                },
            ]
        );
        assert_eq!(decoder.buffered_len(), 0);
    }

    #[test]
    fn canonical_vectors_use_network_byte_order_and_no_implicit_fields() {
        let offer = Frame::NegotiationOffer(
            NegotiationOffer::new(
                VersionRange::new(0, 0).expect("range"),
                CapabilityAdvertisement::new(Capabilities::RELIABLE_STREAM, Capabilities(0))
                    .expect("capabilities"),
                nonce(1),
            )
            .expect("offer"),
        );
        assert_eq!(
            offer.encode().expect("encode"),
            vec![
                // Envelope, version range, and required capabilities.
                0x01, 0x00, 0x00, 0x24, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                0x00, 0x01,
                // Optional capabilities followed by the 16-octet client nonce.
                0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01,
                0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01,
            ]
        );
        let data = Frame::StreamData {
            flow_id: FlowId(1),
            epoch: Epoch(2),
            sequence: Sequence(3),
            bytes: vec![0xaa],
        };
        assert_eq!(
            data.encode().expect("encode"),
            vec![
                0x11, 0x00, 0x00, 0x19, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00,
                0x00, 0x00, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x03,
                0xaa,
            ]
        );
    }

    #[test]
    fn oversized_and_unknown_mandatory_frames_fail_closed() {
        let oversized = [STREAM_DATA, 0, 0, 65];
        assert_eq!(
            Frame::decode_exact(&oversized, 64),
            Err(FrameDecodeError::PayloadTooLarge {
                maximum: 64,
                actual: 65,
            })
        );
        assert_eq!(
            Frame::decode_exact(&[0x40, 0, 0, 0], MAX_FRAME_PAYLOAD),
            Err(FrameDecodeError::UnknownMandatoryFrame(0x40))
        );
        assert_eq!(
            Frame::decode_exact(
                &[STREAM_OPEN, 1, 0, 8, 0, 0, 0, 0, 0, 0, 0, 1],
                MAX_FRAME_PAYLOAD,
            ),
            Err(FrameDecodeError::InvalidPayload)
        );
    }

    #[test]
    fn unknown_optional_frame_round_trips_but_mandatory_optional_bit_is_enforced() {
        let frame = Frame::UnknownOptional {
            frame_type: 0x91,
            payload: vec![1, 2],
        };
        let encoded = frame.encode().expect("encode");
        assert_eq!(Frame::decode_exact(&encoded, 64), Ok(frame));
        assert_eq!(
            Frame::UnknownOptional {
                frame_type: 0x21,
                payload: vec![],
            }
            .encode(),
            Err(FrameEncodeError::InvalidOptionalFrameType(0x21))
        );
    }

    #[test]
    fn empty_data_is_rejected_to_prevent_sequence_flooding() {
        assert_eq!(
            Frame::StreamData {
                flow_id: FlowId(1),
                epoch: Epoch(1),
                sequence: Sequence(1),
                bytes: vec![],
            }
            .encode(),
            Err(FrameEncodeError::EmptyStreamData)
        );
        assert_eq!(
            Frame::decode_exact(
                &[
                    STREAM_DATA,
                    0,
                    0,
                    24,
                    0,
                    0,
                    0,
                    0,
                    0,
                    0,
                    0,
                    1,
                    0,
                    0,
                    0,
                    0,
                    0,
                    0,
                    0,
                    1,
                    0,
                    0,
                    0,
                    0,
                    0,
                    0,
                    0,
                    1,
                ],
                MAX_FRAME_PAYLOAD,
            ),
            Err(FrameDecodeError::EmptyStreamData)
        );
    }
}

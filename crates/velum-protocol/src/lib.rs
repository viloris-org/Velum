//! Carrier-independent v0 protocol types and bounded frame codec.

mod frame;
mod types;

pub use frame::{
    FRAME_HEADER_BYTES, Frame, FrameDecodeError, FrameDecoder, FrameEncodeError, MAX_FRAME_PAYLOAD,
};
pub use types::{
    AttachmentId, AttachmentIdError, Capabilities, CapabilityAdvertisement,
    CapabilityAdvertisementError, Epoch, FlowId, HandshakeNonce, HandshakeNonceError,
    NEGOTIATED_PARAMETERS_BYTES, NegotiatedParameters, NegotiationError, NegotiationOffer,
    Sequence, SessionId, VersionRange, VersionRangeError,
};

/// Stable, payload-free error codes emitted by v0 control frames.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProtocolErrorCode {
    NoSharedVersion,
    IncompatibleCapabilities,
    AuthenticationFailed,
    ReplayDetected,
    ProtocolViolation,
    ResourceLimit,
    FlowNotFound,
    FlowState,
    ResumeUnsupported,
    Internal,
    Unknown(u16),
}

impl ProtocolErrorCode {
    pub const fn as_u16(self) -> u16 {
        match self {
            Self::NoSharedVersion => 0x0001,
            Self::IncompatibleCapabilities => 0x0002,
            Self::AuthenticationFailed => 0x0003,
            Self::ReplayDetected => 0x0004,
            Self::ProtocolViolation => 0x0005,
            Self::ResourceLimit => 0x0006,
            Self::FlowNotFound => 0x0007,
            Self::FlowState => 0x0008,
            Self::ResumeUnsupported => 0x0009,
            Self::Internal => 0x00ff,
            Self::Unknown(value) => value,
        }
    }

    pub const fn from_u16(value: u16) -> Self {
        match value {
            0x0001 => Self::NoSharedVersion,
            0x0002 => Self::IncompatibleCapabilities,
            0x0003 => Self::AuthenticationFailed,
            0x0004 => Self::ReplayDetected,
            0x0005 => Self::ProtocolViolation,
            0x0006 => Self::ResourceLimit,
            0x0007 => Self::FlowNotFound,
            0x0008 => Self::FlowState,
            0x0009 => Self::ResumeUnsupported,
            0x00ff => Self::Internal,
            unknown => Self::Unknown(unknown),
        }
    }
}

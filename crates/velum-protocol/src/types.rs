//! Carrier-independent protocol identifiers and negotiation types.

/// A logical flow identifier, unique within a session and never reused.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct FlowId(pub u64);

/// Opaque identifier for one logical session.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SessionId(pub [u8; 16]);

/// Monotonically increasing carrier-transition generation for a session.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Epoch(pub u64);

/// Monotonically increasing reliable-segment sequence within one flow.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Sequence(pub u64);

/// An inclusive range of supported protocol versions.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VersionRange {
    pub minimum: u16,
    pub maximum: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VersionRangeError {
    InvalidRange,
}

impl VersionRange {
    pub const fn new(minimum: u16, maximum: u16) -> Result<Self, VersionRangeError> {
        if minimum > maximum {
            return Err(VersionRangeError::InvalidRange);
        }
        Ok(Self { minimum, maximum })
    }

    pub const fn is_valid(self) -> bool {
        self.minimum <= self.maximum
    }

    pub const fn negotiate(self, peer: Self) -> Option<u16> {
        if !self.is_valid() || !peer.is_valid() {
            return None;
        }
        let minimum = if self.minimum > peer.minimum {
            self.minimum
        } else {
            peer.minimum
        };
        let maximum = if self.maximum < peer.maximum {
            self.maximum
        } else {
            peer.maximum
        };
        if minimum <= maximum {
            Some(maximum)
        } else {
            None
        }
    }
}

/// Bitset of carrier-independent features understood by a peer.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Capabilities(pub u64);

impl Capabilities {
    pub const RELIABLE_STREAM: Self = Self(1 << 0);
    pub const UNRELIABLE_DATAGRAM: Self = Self(1 << 1);
    pub const KNOWN: Self = Self(Self::RELIABLE_STREAM.0 | Self::UNRELIABLE_DATAGRAM.0);

    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    pub const fn intersects(self, other: Self) -> bool {
        self.0 & other.0 != 0
    }

    pub const fn intersection(self, other: Self) -> Self {
        Self(self.0 & other.0)
    }

    pub const fn unknown(self) -> Self {
        Self(self.0 & !Self::KNOWN.0)
    }
}

/// A capability offer separates required behavior from optional behavior.
///
/// Unknown required bits reject negotiation. Unknown optional bits are not
/// enabled, allowing a newer peer to interoperate with an older one.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CapabilityAdvertisement {
    pub required: Capabilities,
    pub optional: Capabilities,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CapabilityAdvertisementError {
    OverlappingBits,
}

impl CapabilityAdvertisement {
    pub const fn new(
        required: Capabilities,
        optional: Capabilities,
    ) -> Result<Self, CapabilityAdvertisementError> {
        if required.intersects(optional) {
            return Err(CapabilityAdvertisementError::OverlappingBits);
        }
        Ok(Self { required, optional })
    }

    pub const fn offered(self) -> Capabilities {
        Capabilities(self.required.0 | self.optional.0)
    }
}

/// A non-zero nonce that binds a negotiation response to its offer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HandshakeNonce([u8; 16]);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HandshakeNonceError {
    AllZero,
}

impl HandshakeNonce {
    pub const fn new(bytes: [u8; 16]) -> Result<Self, HandshakeNonceError> {
        let mut index = 0;
        while index < bytes.len() {
            if bytes[index] != 0 {
                return Ok(Self(bytes));
            }
            index += 1;
        }
        Err(HandshakeNonceError::AllZero)
    }

    pub const fn bytes(self) -> [u8; 16] {
        self.0
    }
}

/// A fresh opaque identifier for one carrier-attachment attempt.
///
/// It is protocol data, unlike a process-local carrier handle, and is echoed
/// by `AttachAccepted` so a lost acknowledgement can be retried idempotently.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AttachmentId([u8; 16]);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AttachmentIdError {
    AllZero,
}

impl AttachmentId {
    pub const fn new(bytes: [u8; 16]) -> Result<Self, AttachmentIdError> {
        let mut index = 0;
        while index < bytes.len() {
            if bytes[index] != 0 {
                return Ok(Self(bytes));
            }
            index += 1;
        }
        Err(AttachmentIdError::AllZero)
    }

    pub const fn bytes(self) -> [u8; 16] {
        self.0
    }
}

/// A client's authenticated offer before a logical session is attached.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NegotiationOffer {
    pub versions: VersionRange,
    pub capabilities: CapabilityAdvertisement,
    pub client_nonce: HandshakeNonce,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NegotiationError {
    InvalidVersionRange,
    InvalidCapabilityAdvertisement,
    NoSharedVersion,
    UnsupportedRequiredCapability,
    MissingReliableStream,
    InvalidSelection,
}

impl NegotiationOffer {
    pub const fn new(
        versions: VersionRange,
        capabilities: CapabilityAdvertisement,
        client_nonce: HandshakeNonce,
    ) -> Result<Self, NegotiationError> {
        if !versions.is_valid() {
            return Err(NegotiationError::InvalidVersionRange);
        }
        if capabilities.required.intersects(capabilities.optional) {
            return Err(NegotiationError::InvalidCapabilityAdvertisement);
        }
        if !capabilities
            .required
            .contains(Capabilities::RELIABLE_STREAM)
        {
            return Err(NegotiationError::MissingReliableStream);
        }
        Ok(Self {
            versions,
            capabilities,
            client_nonce,
        })
    }

    /// Selects the newest common version and every mutually supported offered
    /// capability. A v0 peer always requires reliable-stream support.
    pub const fn negotiate(
        self,
        server_versions: VersionRange,
        server_capabilities: Capabilities,
        server_nonce: HandshakeNonce,
    ) -> Result<NegotiatedParameters, NegotiationError> {
        let version = match self.versions.negotiate(server_versions) {
            Some(version) => version,
            None => return Err(NegotiationError::NoSharedVersion),
        };
        if !server_capabilities.contains(self.capabilities.required) {
            return Err(NegotiationError::UnsupportedRequiredCapability);
        }
        let enabled = self
            .capabilities
            .offered()
            .intersection(server_capabilities);
        if !enabled.contains(Capabilities::RELIABLE_STREAM) {
            return Err(NegotiationError::MissingReliableStream);
        }
        Ok(NegotiatedParameters {
            version,
            capabilities: enabled,
            client_nonce: self.client_nonce,
            server_nonce,
        })
    }
}

/// The accepted version, capabilities, and nonces. Its canonical bytes are
/// included in every carrier-attachment proof.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NegotiatedParameters {
    pub version: u16,
    pub capabilities: Capabilities,
    pub client_nonce: HandshakeNonce,
    pub server_nonce: HandshakeNonce,
}

pub const NEGOTIATED_PARAMETERS_BYTES: usize = 42;

impl NegotiatedParameters {
    pub fn validate_for(self, offer: NegotiationOffer) -> Result<(), NegotiationError> {
        if !offer.versions.is_valid()
            || self.version < offer.versions.minimum
            || self.version > offer.versions.maximum
            || !offer.capabilities.offered().contains(self.capabilities)
            || !self.capabilities.contains(offer.capabilities.required)
            || !self.capabilities.contains(Capabilities::RELIABLE_STREAM)
            || self.client_nonce.0 != offer.client_nonce.0
        {
            return Err(NegotiationError::InvalidSelection);
        }
        Ok(())
    }

    /// Canonical, fixed-width representation for framing and transcript
    /// binding. All integers use network byte order.
    pub const fn canonical_bytes(self) -> [u8; NEGOTIATED_PARAMETERS_BYTES] {
        let mut bytes = [0; NEGOTIATED_PARAMETERS_BYTES];
        let version = self.version.to_be_bytes();
        let capabilities = self.capabilities.0.to_be_bytes();
        let client_nonce = self.client_nonce.bytes();
        let server_nonce = self.server_nonce.bytes();
        bytes[0] = version[0];
        bytes[1] = version[1];
        bytes[2] = capabilities[0];
        bytes[3] = capabilities[1];
        bytes[4] = capabilities[2];
        bytes[5] = capabilities[3];
        bytes[6] = capabilities[4];
        bytes[7] = capabilities[5];
        bytes[8] = capabilities[6];
        bytes[9] = capabilities[7];
        let mut index = 0;
        while index < client_nonce.len() {
            bytes[10 + index] = client_nonce[index];
            bytes[26 + index] = server_nonce[index];
            index += 1;
        }
        bytes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const fn nonce(byte: u8) -> HandshakeNonce {
        match HandshakeNonce::new([byte; 16]) {
            Ok(nonce) => nonce,
            Err(_) => panic!("non-zero nonce"),
        }
    }

    #[test]
    fn invalid_ranges_cannot_negotiate() {
        assert_eq!(
            VersionRange::new(2, 1),
            Err(VersionRangeError::InvalidRange)
        );
        assert_eq!(
            VersionRange {
                minimum: 2,
                maximum: 1,
            }
            .negotiate(VersionRange {
                minimum: 0,
                maximum: 3,
            }),
            None
        );
    }

    #[test]
    fn required_capabilities_fail_closed_and_optional_capabilities_downgrade_cleanly() {
        assert_eq!(
            NegotiationOffer::new(
                VersionRange::new(0, 0).expect("range"),
                CapabilityAdvertisement::new(Capabilities(0), Capabilities::RELIABLE_STREAM)
                    .expect("capabilities"),
                nonce(1),
            ),
            Err(NegotiationError::MissingReliableStream)
        );
        let offer = NegotiationOffer::new(
            VersionRange::new(0, 2).expect("range"),
            CapabilityAdvertisement::new(
                Capabilities::RELIABLE_STREAM,
                Capabilities::UNRELIABLE_DATAGRAM,
            )
            .expect("capabilities"),
            nonce(1),
        )
        .expect("offer");

        assert_eq!(
            offer.negotiate(
                VersionRange::new(1, 3).expect("range"),
                Capabilities::RELIABLE_STREAM,
                nonce(2),
            ),
            Ok(NegotiatedParameters {
                version: 2,
                capabilities: Capabilities::RELIABLE_STREAM,
                client_nonce: nonce(1),
                server_nonce: nonce(2),
            })
        );

        assert_eq!(
            offer.negotiate(
                VersionRange::new(1, 3).expect("range"),
                Capabilities::UNRELIABLE_DATAGRAM,
                nonce(2),
            ),
            Err(NegotiationError::UnsupportedRequiredCapability)
        );
    }

    #[test]
    fn response_validation_rejects_changed_nonce_or_capability() {
        let offer = NegotiationOffer::new(
            VersionRange::new(0, 0).expect("range"),
            CapabilityAdvertisement::new(Capabilities::RELIABLE_STREAM, Capabilities(0))
                .expect("capabilities"),
            nonce(3),
        )
        .expect("offer");
        let selected = offer
            .negotiate(
                VersionRange::new(0, 0).expect("range"),
                Capabilities::RELIABLE_STREAM,
                nonce(4),
            )
            .expect("selection");
        assert_eq!(selected.validate_for(offer), Ok(()));
        assert_eq!(
            NegotiatedParameters {
                client_nonce: nonce(5),
                ..selected
            }
            .validate_for(offer),
            Err(NegotiationError::InvalidSelection)
        );
    }
}

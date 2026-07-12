//! Cryptographic state boundaries for v0 carrier attachment.

use ring::hmac;
use velum_protocol::{AttachmentId, Epoch, NegotiatedParameters, SessionId};

const ATTACHMENT_KEY_LABEL: &[u8] = b"velum v0 attachment key";
const ATTACHMENT_CONTEXT_LABEL: &[u8] = b"velum v0 carrier attachment";
pub const ATTACHMENT_PROOF_BYTES: usize = 32;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AttachmentProof([u8; ATTACHMENT_PROOF_BYTES]);

impl AttachmentProof {
    pub const fn from_bytes(bytes: [u8; ATTACHMENT_PROOF_BYTES]) -> Self {
        Self(bytes)
    }

    pub const fn bytes(self) -> [u8; ATTACHMENT_PROOF_BYTES] {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AttachmentAuthenticationError {
    EmptySecret,
    InvalidProof,
}

/// Creates and verifies proofs that bind a carrier to one logical session,
/// epoch, and authenticated negotiation transcript.
///
/// The constructor takes a session-specific authentication secret. Its caller
/// owns initial authentication and secret rotation; this type derives a
/// domain-separated key only for attachment proofs.
pub struct AttachmentAuthenticator {
    key: hmac::Key,
}

impl AttachmentAuthenticator {
    pub fn new(secret: &[u8]) -> Result<Self, AttachmentAuthenticationError> {
        if secret.is_empty() {
            return Err(AttachmentAuthenticationError::EmptySecret);
        }
        let root = hmac::Key::new(hmac::HMAC_SHA256, secret);
        let derived = hmac::sign(&root, ATTACHMENT_KEY_LABEL);
        Ok(Self {
            key: hmac::Key::new(hmac::HMAC_SHA256, derived.as_ref()),
        })
    }

    pub fn prove(
        &self,
        session: SessionId,
        epoch: Epoch,
        attachment_id: AttachmentId,
        parameters: NegotiatedParameters,
    ) -> AttachmentProof {
        let tag = hmac::sign(
            &self.key,
            &attachment_context(session, epoch, attachment_id, parameters),
        );
        let mut proof = [0; ATTACHMENT_PROOF_BYTES];
        proof.copy_from_slice(tag.as_ref());
        AttachmentProof(proof)
    }

    pub fn verify(
        &self,
        session: SessionId,
        epoch: Epoch,
        attachment_id: AttachmentId,
        parameters: NegotiatedParameters,
        proof: AttachmentProof,
    ) -> Result<(), AttachmentAuthenticationError> {
        hmac::verify(
            &self.key,
            &attachment_context(session, epoch, attachment_id, parameters),
            &proof.0,
        )
        .map_err(|_| AttachmentAuthenticationError::InvalidProof)
    }
}

fn attachment_context(
    session: SessionId,
    epoch: Epoch,
    attachment_id: AttachmentId,
    parameters: NegotiatedParameters,
) -> Vec<u8> {
    let mut context = Vec::with_capacity(
        ATTACHMENT_CONTEXT_LABEL.len() + 16 + 8 + 16 + velum_protocol::NEGOTIATED_PARAMETERS_BYTES,
    );
    context.extend_from_slice(ATTACHMENT_CONTEXT_LABEL);
    context.extend_from_slice(&session.0);
    context.extend_from_slice(&epoch.0.to_be_bytes());
    context.extend_from_slice(&attachment_id.bytes());
    context.extend_from_slice(&parameters.canonical_bytes());
    context
}

/// Records the most recent accepted epoch. Proof verification is introduced
/// with carrier attachment; the session remains its sole caller.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ReplayWindow(Option<Epoch>);

impl ReplayWindow {
    pub fn is_empty(&self) -> bool {
        self.0.is_none()
    }

    pub fn accepts(&self, epoch: Epoch) -> bool {
        self.0.is_none_or(|latest| epoch > latest)
    }

    pub fn accept(&mut self, epoch: Epoch) -> bool {
        if !self.accepts(epoch) {
            return false;
        }
        self.0 = Some(epoch);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use velum_protocol::{
        AttachmentId, Capabilities, CapabilityAdvertisement, HandshakeNonce, NegotiationOffer,
        VersionRange,
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
    fn attachment_proof_binds_session_epoch_attempt_and_negotiation() {
        let authenticator = AttachmentAuthenticator::new(b"attachment secret").expect("secret");
        let session = SessionId([7; 16]);
        let parameters = parameters();
        let attachment_id = AttachmentId::new([4; 16]).expect("attachment id");
        let proof = authenticator.prove(session, Epoch(9), attachment_id, parameters);

        assert_eq!(
            authenticator.verify(session, Epoch(9), attachment_id, parameters, proof),
            Ok(())
        );
        assert_eq!(
            authenticator.verify(
                session,
                Epoch(9),
                AttachmentId::new([5; 16]).expect("attachment id"),
                parameters,
                proof,
            ),
            Err(AttachmentAuthenticationError::InvalidProof)
        );
        assert_eq!(
            authenticator.verify(
                SessionId([8; 16]),
                Epoch(9),
                attachment_id,
                parameters,
                proof,
            ),
            Err(AttachmentAuthenticationError::InvalidProof)
        );
        assert_eq!(
            authenticator.verify(session, Epoch(10), attachment_id, parameters, proof),
            Err(AttachmentAuthenticationError::InvalidProof)
        );
        assert_eq!(
            authenticator.verify(
                session,
                Epoch(9),
                attachment_id,
                NegotiatedParameters {
                    capabilities: Capabilities(
                        Capabilities::RELIABLE_STREAM.0 | Capabilities::UNRELIABLE_DATAGRAM.0,
                    ),
                    ..parameters
                },
                proof,
            ),
            Err(AttachmentAuthenticationError::InvalidProof)
        );
    }

    #[test]
    fn proof_has_a_fixed_wire_representation() {
        let authenticator = AttachmentAuthenticator::new(b"attachment secret").expect("secret");
        let proof = authenticator.prove(
            SessionId([7; 16]),
            Epoch(9),
            AttachmentId::new([4; 16]).expect("attachment id"),
            parameters(),
        );
        assert_eq!(AttachmentProof::from_bytes(proof.bytes()), proof);
    }
}

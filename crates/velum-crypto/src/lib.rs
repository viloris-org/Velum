//! Cryptographic state boundaries for the tracer.

use ring::hmac;
use velum_protocol::{Epoch, SessionId};

const ATTACHMENT_LABEL: &[u8] = b"velum carrier attachment v1";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AttachmentProof([u8; 32]);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AttachmentAuthenticationError {
    EmptySecret,
    InvalidProof,
}

/// Creates and verifies proofs that bind a carrier to one logical session and
/// epoch. This is an in-process contract; a Stage 5 frame codec must define
/// the serialized representation separately.
pub struct AttachmentAuthenticator {
    key: hmac::Key,
}

impl AttachmentAuthenticator {
    pub fn new(secret: &[u8]) -> Result<Self, AttachmentAuthenticationError> {
        if secret.is_empty() {
            return Err(AttachmentAuthenticationError::EmptySecret);
        }
        Ok(Self {
            key: hmac::Key::new(hmac::HMAC_SHA256, secret),
        })
    }

    pub fn prove(&self, session: SessionId, carrier: u64, epoch: Epoch) -> AttachmentProof {
        let tag = hmac::sign(&self.key, &attachment_context(session, carrier, epoch));
        let mut proof = [0; 32];
        proof.copy_from_slice(tag.as_ref());
        AttachmentProof(proof)
    }

    pub fn verify(
        &self,
        session: SessionId,
        carrier: u64,
        epoch: Epoch,
        proof: AttachmentProof,
    ) -> Result<(), AttachmentAuthenticationError> {
        hmac::verify(
            &self.key,
            &attachment_context(session, carrier, epoch),
            &proof.0,
        )
        .map_err(|_| AttachmentAuthenticationError::InvalidProof)
    }
}

fn attachment_context(session: SessionId, carrier: u64, epoch: Epoch) -> Vec<u8> {
    let mut context = Vec::with_capacity(ATTACHMENT_LABEL.len() + 16 + 8 + 8);
    context.extend_from_slice(ATTACHMENT_LABEL);
    context.extend_from_slice(&session.0);
    context.extend_from_slice(&carrier.to_be_bytes());
    context.extend_from_slice(&epoch.0.to_be_bytes());
    context
}

/// Records the most recent accepted epoch. Proof verification is introduced
/// with carrier attachment; the session remains its sole caller.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ReplayWindow(Option<Epoch>);

impl ReplayWindow {
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

    #[test]
    fn attachment_proof_binds_session_carrier_and_epoch() {
        let authenticator = AttachmentAuthenticator::new(b"attachment secret").expect("secret");
        let session = SessionId([7; 16]);
        let proof = authenticator.prove(session, 4, Epoch(9));

        assert_eq!(authenticator.verify(session, 4, Epoch(9), proof), Ok(()));
        assert_eq!(
            authenticator.verify(session, 5, Epoch(9), proof),
            Err(AttachmentAuthenticationError::InvalidProof)
        );
        assert_eq!(
            authenticator.verify(SessionId([8; 16]), 4, Epoch(9), proof),
            Err(AttachmentAuthenticationError::InvalidProof)
        );
        assert_eq!(
            authenticator.verify(session, 4, Epoch(10), proof),
            Err(AttachmentAuthenticationError::InvalidProof)
        );
    }
}

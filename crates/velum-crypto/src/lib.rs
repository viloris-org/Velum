//! Cryptographic state boundaries for the tracer.

use velum_protocol::Epoch;

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

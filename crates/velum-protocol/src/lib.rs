//! Protocol-level identifiers and state-machine types.
//!
//! Frame encoding is deliberately deferred until the session tracer has
//! established the required state transitions.

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct FlowId(pub u64);

/// Opaque identifier for one logical session.
///
/// Its byte representation is intentionally not a wire-format commitment.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SessionId(pub [u8; 16]);

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Epoch(pub u64);

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Sequence(pub u64);

/// Version range advertised before a logical-session attachment is accepted.
/// This is typed negotiation state, not a Stage 5 wire encoding commitment.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VersionRange {
    pub minimum: u16,
    pub maximum: u16,
}

impl VersionRange {
    pub const fn negotiate(self, peer: Self) -> Option<u16> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_negotiation_selects_the_newest_shared_version() {
        assert_eq!(
            VersionRange {
                minimum: 0,
                maximum: 2
            }
            .negotiate(VersionRange {
                minimum: 1,
                maximum: 3
            }),
            Some(2)
        );
        assert_eq!(
            VersionRange {
                minimum: 0,
                maximum: 0
            }
            .negotiate(VersionRange {
                minimum: 1,
                maximum: 2
            }),
            None
        );
    }
}

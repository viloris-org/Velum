//! Placement recommendations are values, never session mutations.

mod transition;

use velum_carrier_api::Reliability;

pub use transition::{
    CarrierObservation, FallbackMode, TransitionDecision, TransitionPolicy, TransitionReason,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FlowRequirement {
    pub reliability: Reliability,
}

/// TLS carries reliable streams only. A caller must preserve or explicitly
/// reject datagram semantics instead of silently encapsulating them.
pub fn fallback_supports(requirement: FlowRequirement) -> bool {
    requirement.reliability == Reliability::Stream
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tls_fallback_never_claims_datagram_support() {
        assert!(fallback_supports(FlowRequirement {
            reliability: Reliability::Stream,
        }));
        assert!(!fallback_supports(FlowRequirement {
            reliability: Reliability::Datagram,
        }));
    }
}

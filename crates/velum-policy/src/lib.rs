//! Placement recommendations are values, never session mutations.

use velum_carrier_api::Reliability;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FlowRequirement {
    pub reliability: Reliability,
}

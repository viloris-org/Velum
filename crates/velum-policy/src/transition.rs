use velum_carrier_api::{CarrierHealth, CarrierKind};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FallbackMode {
    Cold,
    Warm,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TransitionReason {
    PrimaryUnhealthy,
    PrimaryRecovered,
    LossThresholdExceeded,
    LatencyThresholdExceeded,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TransitionDecision {
    Stay,
    Transition {
        mode: FallbackMode,
        reason: TransitionReason,
        to: CarrierKind,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CarrierObservation {
    pub kind: CarrierKind,
    pub health: CarrierHealth,
    pub at_millis: u64,
}

/// Hysteresis and rate limits for carrier changes. The policy is pure so its
/// decision can be replayed from redacted observations.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TransitionPolicy {
    pub fallback_mode: FallbackMode,
    pub unhealthy_observations: u32,
    pub recovery_observations: u32,
    pub minimum_transition_interval_millis: u64,
    pub maximum_loss_parts_per_million: u32,
    pub maximum_round_trip_time_millis: u64,
}

impl TransitionPolicy {
    pub fn decide(
        self,
        active: CarrierKind,
        primary: CarrierObservation,
        fallback: CarrierObservation,
        consecutive_unhealthy: u32,
        consecutive_recovered: u32,
        last_transition_at_millis: Option<u64>,
    ) -> TransitionDecision {
        if active == CarrierKind::Quic
            && (fallback.kind != CarrierKind::Tls || !fallback.health.is_healthy)
        {
            return TransitionDecision::Stay;
        }
        if last_transition_at_millis.is_some_and(|last| {
            primary.at_millis.saturating_sub(last) < self.minimum_transition_interval_millis
        }) {
            return TransitionDecision::Stay;
        }

        let reason = if active == CarrierKind::Quic
            && !primary.health.is_healthy
            && consecutive_unhealthy >= self.unhealthy_observations
        {
            Some((TransitionReason::PrimaryUnhealthy, CarrierKind::Tls))
        } else if active == CarrierKind::Quic
            && primary
                .health
                .loss_parts_per_million
                .is_some_and(|loss| loss > self.maximum_loss_parts_per_million)
            && consecutive_unhealthy >= self.unhealthy_observations
        {
            Some((TransitionReason::LossThresholdExceeded, CarrierKind::Tls))
        } else if active == CarrierKind::Quic
            && primary
                .health
                .round_trip_time_millis
                .is_some_and(|rtt| rtt > self.maximum_round_trip_time_millis)
            && consecutive_unhealthy >= self.unhealthy_observations
        {
            Some((TransitionReason::LatencyThresholdExceeded, CarrierKind::Tls))
        } else if active == CarrierKind::Tls
            && primary.health.is_healthy
            && consecutive_recovered >= self.recovery_observations
        {
            Some((TransitionReason::PrimaryRecovered, CarrierKind::Quic))
        } else {
            None
        };
        reason.map_or(TransitionDecision::Stay, |(reason, to)| {
            TransitionDecision::Transition {
                mode: self.fallback_mode,
                reason,
                to,
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn observation(kind: CarrierKind, healthy: bool, at_millis: u64) -> CarrierObservation {
        CarrierObservation {
            kind,
            health: CarrierHealth {
                round_trip_time_millis: Some(10),
                loss_parts_per_million: Some(0),
                is_healthy: healthy,
            },
            at_millis,
        }
    }

    fn policy(mode: FallbackMode) -> TransitionPolicy {
        TransitionPolicy {
            fallback_mode: mode,
            unhealthy_observations: 3,
            recovery_observations: 2,
            minimum_transition_interval_millis: 100,
            maximum_loss_parts_per_million: 1_000,
            maximum_round_trip_time_millis: 100,
        }
    }

    #[test]
    fn cold_and_warm_fallbacks_require_confirmed_failure() {
        for mode in [FallbackMode::Cold, FallbackMode::Warm] {
            assert_eq!(
                policy(mode).decide(
                    CarrierKind::Quic,
                    observation(CarrierKind::Quic, false, 200),
                    observation(CarrierKind::Tls, true, 200),
                    3,
                    0,
                    None,
                ),
                TransitionDecision::Transition {
                    mode,
                    reason: TransitionReason::PrimaryUnhealthy,
                    to: CarrierKind::Tls,
                }
            );
        }
    }

    #[test]
    fn policy_controls_false_transitions_and_recovery_flaps() {
        let primary = observation(CarrierKind::Quic, false, 150);
        let fallback = observation(CarrierKind::Tls, true, 150);
        assert_eq!(
            policy(FallbackMode::Cold).decide(CarrierKind::Quic, primary, fallback, 2, 0, None),
            TransitionDecision::Stay
        );
        assert_eq!(
            policy(FallbackMode::Cold).decide(
                CarrierKind::Quic,
                primary,
                fallback,
                3,
                0,
                Some(100),
            ),
            TransitionDecision::Stay
        );
        assert_eq!(
            policy(FallbackMode::Cold).decide(
                CarrierKind::Tls,
                observation(CarrierKind::Quic, true, 250),
                fallback,
                0,
                2,
                Some(100),
            ),
            TransitionDecision::Transition {
                mode: FallbackMode::Cold,
                reason: TransitionReason::PrimaryRecovered,
                to: CarrierKind::Quic,
            }
        );
    }

    #[test]
    fn recovery_does_not_depend_on_the_failed_tls_fallback() {
        assert_eq!(
            policy(FallbackMode::Cold).decide(
                CarrierKind::Tls,
                observation(CarrierKind::Quic, true, 250),
                observation(CarrierKind::Tls, false, 250),
                0,
                2,
                None,
            ),
            TransitionDecision::Transition {
                mode: FallbackMode::Cold,
                reason: TransitionReason::PrimaryRecovered,
                to: CarrierKind::Quic,
            }
        );
    }
}

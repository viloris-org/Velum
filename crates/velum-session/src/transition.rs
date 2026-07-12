//! Bounded epoch validity during a carrier transition.

use velum_protocol::Epoch;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct TransitionState {
    current: Epoch,
    retiring: Option<Epoch>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum EpochValidity {
    Current,
    Retiring,
    Stale,
    Future,
}

impl TransitionState {
    pub(crate) fn new(current: Epoch) -> Self {
        Self {
            current,
            retiring: None,
        }
    }

    pub(crate) fn current(&self) -> Epoch {
        self.current
    }

    pub(crate) fn advance(&mut self) -> Option<Epoch> {
        if self.retiring.is_some() {
            return None;
        }
        let next = Epoch(self.current.0.checked_add(1).expect("epoch exhausted"));
        self.retiring = Some(self.current);
        self.current = next;
        Some(next)
    }

    pub(crate) fn retire_previous(&mut self) {
        self.retiring = None;
    }

    pub(crate) fn validate(&self, epoch: Epoch) -> EpochValidity {
        if epoch == self.current {
            EpochValidity::Current
        } else if Some(epoch) == self.retiring {
            EpochValidity::Retiring
        } else if epoch < self.current {
            EpochValidity::Stale
        } else {
            EpochValidity::Future
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_current_and_one_retiring_epoch_are_valid() {
        let mut state = TransitionState::new(Epoch(4));
        assert_eq!(state.validate(Epoch(4)), EpochValidity::Current);
        assert_eq!(state.validate(Epoch(3)), EpochValidity::Stale);

        assert_eq!(state.advance(), Some(Epoch(5)));
        assert_eq!(state.validate(Epoch(4)), EpochValidity::Retiring);
        assert_eq!(state.validate(Epoch(5)), EpochValidity::Current);
        assert_eq!(state.validate(Epoch(3)), EpochValidity::Stale);
        assert_eq!(state.validate(Epoch(6)), EpochValidity::Future);

        state.retire_previous();
        assert_eq!(state.validate(Epoch(4)), EpochValidity::Stale);
    }
}

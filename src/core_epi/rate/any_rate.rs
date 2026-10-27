use super::{ConstantRateFn, EmpiricalRateFn, InvertibleRate};

/// Any concrete infectiousness rate function.
///
/// An enum rather than `Box<dyn InvertibleRate>`: the transmission loop calls
/// `rate`/`cum_rate`/`inverse_cum_rate` per event, and static dispatch keeps
/// those inlinable.
#[derive(Clone, Debug)]
pub enum AnyInfectionRate {
    Constant(ConstantRateFn),
    Empirical(EmpiricalRateFn),
}

impl From<ConstantRateFn> for AnyInfectionRate {
    fn from(rate: ConstantRateFn) -> Self {
        Self::Constant(rate)
    }
}

impl From<EmpiricalRateFn> for AnyInfectionRate {
    fn from(rate: EmpiricalRateFn) -> Self {
        Self::Empirical(rate)
    }
}

macro_rules! dispatch {
    ($self:ident, $r:ident => $e:expr) => {
        match $self {
            Self::Constant($r) => $e,
            Self::Empirical($r) => $e,
        }
    };
}

impl InvertibleRate for AnyInfectionRate {
    #[inline]
    fn rate(&self, t: f64) -> f64 {
        dispatch!(self, r => r.rate(t))
    }

    #[inline]
    fn cum_rate(&self, t: f64) -> f64 {
        dispatch!(self, r => r.cum_rate(t))
    }

    #[inline]
    fn inverse_cum_rate(&self, events: f64) -> Option<f64> {
        dispatch!(self, r => r.inverse_cum_rate(events))
    }

    #[inline]
    fn max_cum_rate(&self) -> f64 {
        dispatch!(self, r => r.max_cum_rate())
    }
}

impl AnyInfectionRate {
    /// Time from the start of the curve until its integrated infectiousness is
    /// exhausted: the period of a constant rate, the earliest time at which an
    /// empirical curve's cumulative peaks.
    #[must_use]
    pub fn duration(&self) -> f64 {
        dispatch!(self, r => r.duration())
    }
}

use super::{ConstantRateFn, EmpiricalRateFn, InvertibleRate};

/// Any concrete infectiousness rate function. An enum rather than a boxed
/// trait object so the loop's per-event calls stay inlinable.
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
}

impl AnyInfectionRate {
    /// Time from the start of the curve until its infectiousness is exhausted.
    #[must_use]
    pub fn duration(&self) -> f64 {
        dispatch!(self, r => r.duration())
    }
}

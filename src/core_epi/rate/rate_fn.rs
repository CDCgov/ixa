/// Utility functions for calculating the rate of infection over time
/// See `TransformedInvertibleRateFn` for the inverse cumulative rate of an interval starting
/// at a time other than 0.
pub trait InvertibleRate {
    /// Returns the rate at time `t`
    fn rate(&self, t: f64) -> f64;

    /// Returns the expected number of events in the interval 0 -> t
    fn cum_rate(&self, t: f64) -> f64;

    /// Returns the expected time, starting at 0, at which a number of infection `events` will have
    /// occurred. Returns `None` if `events` is greater than [`cum_rate`](Self::cum_rate)'s maximum value.
    fn inverse_cum_rate(&self, events: f64) -> Option<f64>;

    /// The maximum cumulative infectiousness this rate function can ever produce —
    /// `cum_rate(+∞)`, i.e. the total integrated area under `rate`. It is the value at
    /// which [`inverse_cum_rate`](Self::inverse_cum_rate) saturates to `None` (no more
    /// events are possible). A time-unbounded rate (e.g. a constant hazard) returns
    /// `f64::INFINITY`; an empirical curve returns its finite total area.
    fn max_cum_rate(&self) -> f64;
}

/// A utility for a rate function that has been scaled or shifted (i.e., affine transformed)
pub struct TransformedInvertibleRateFn<'a, T>
where
    T: InvertibleRate + ?Sized,
{
    pub base: &'a T,
    pub scale: f64,
    pub elapsed: f64,
}

impl<'a, T: ?Sized + InvertibleRate> TransformedInvertibleRateFn<'a, T> {
    #[must_use]
    pub fn new(base: &'a T, scale: f64, elapsed: f64) -> Self {
        Self {
            base,
            scale,
            elapsed,
        }
    }
}

/// An invertible rate function multiplied by a constant `factor`.
pub struct ScaledRateFn<F: InvertibleRate> {
    pub inner: F,
    pub factor: f64,
}

impl<F: InvertibleRate> ScaledRateFn<F> {
    #[must_use]
    pub fn new(inner: F, factor: f64) -> Self {
        Self { inner, factor }
    }
}

impl<F: InvertibleRate> InvertibleRate for ScaledRateFn<F> {
    fn rate(&self, t: f64) -> f64 {
        self.inner.rate(t) * self.factor
    }

    fn cum_rate(&self, t: f64) -> f64 {
        self.inner.cum_rate(t) * self.factor
    }

    /// A non-positive `factor` means no events are ever reached.
    fn inverse_cum_rate(&self, events: f64) -> Option<f64> {
        if self.factor <= 0.0 {
            return (events <= 0.0).then_some(0.0);
        }
        self.inner.inverse_cum_rate(events / self.factor)
    }

    fn max_cum_rate(&self) -> f64 {
        if self.factor <= 0.0 {
            return 0.0;
        }
        self.inner.max_cum_rate() * self.factor
    }
}

impl<T: ?Sized + InvertibleRate> InvertibleRate for TransformedInvertibleRateFn<'_, T> {
    fn rate(&self, t: f64) -> f64 {
        self.base.rate(t + self.elapsed) * self.scale
    }

    fn cum_rate(&self, t: f64) -> f64 {
        (self.base.cum_rate(t + self.elapsed) - self.base.cum_rate(self.elapsed)) * self.scale
    }

    /// A non-positive `scale` means no events are ever reached, rather than
    /// `events / 0 = inf`.
    fn inverse_cum_rate(&self, events: f64) -> Option<f64> {
        if self.scale <= 0.0 {
            return (events <= 0.0).then_some(0.0);
        }
        let elapsed_cum_rate = self.base.cum_rate(self.elapsed);
        Some(
            self.base
                .inverse_cum_rate(events / self.scale + elapsed_cum_rate)?
                - self.elapsed,
        )
    }

    /// Propagates `f64::INFINITY` from an unbounded base.
    fn max_cum_rate(&self) -> f64 {
        if self.scale <= 0.0 {
            return 0.0;
        }
        (self.base.max_cum_rate() - self.base.cum_rate(self.elapsed)) * self.scale
    }
}

#[cfg(test)]
mod test {
    use super::{InvertibleRate, ScaledRateFn, TransformedInvertibleRateFn};
    use crate::assert_almost_eq;
    use crate::core_epi::rate::{ConstantRateFn, EmpiricalRateFn};

    // A flat empirical curve of height 2.0 on [0, 5] (rate 0 outside that range) — a
    // finite-support stand-in for a "constant 2.0 for 5 days" profile. Unlike the
    // reference's capped `ConstantRate`, this repo's `ConstantRate` is unbounded in
    // time, so the elapsed/exhaustion behavior is exercised here via `EmpiricalRate`.
    fn flat_capped() -> EmpiricalRateFn {
        EmpiricalRateFn::new(vec![[0.0, 2.0], [5.0, 2.0]], 1.0)
    }

    #[test]
    fn test_scale_rate_fn() {
        let rate_fn = flat_capped();
        let scaled_rate_fn = TransformedInvertibleRateFn {
            base: &rate_fn,
            scale: 2.0,
            elapsed: 0.0,
        };
        assert_almost_eq!(scaled_rate_fn.rate(0.0), 4.0, 0.0);
        assert_almost_eq!(scaled_rate_fn.rate(5.0), 4.0, 0.0);
    }
    #[test]
    fn test_scale_rate_fn_with_elapsed() {
        let rate_fn = flat_capped();
        let scaled_rate_fn = TransformedInvertibleRateFn {
            base: &rate_fn,
            scale: 2.0,
            elapsed: 3.0,
        };
        assert_almost_eq!(scaled_rate_fn.rate(0.0), 4.0, 0.0);
        // Since the elapsed is 3.0, the rate at t=4.0 is past the total infectious
        // period (3.0 + 4.0 = 7.0), so the rate is 0.0.
        assert_almost_eq!(scaled_rate_fn.rate(4.0), 0.0, 0.0);
    }
    #[test]
    fn test_scale_rate_fn_cum_rate() {
        let rate_fn = flat_capped();
        let scaled_rate_fn = TransformedInvertibleRateFn {
            base: &rate_fn,
            scale: 2.0,
            elapsed: 3.0,
        };
        assert_almost_eq!(scaled_rate_fn.cum_rate(1.0), 4.0, 0.0);
        assert_almost_eq!(scaled_rate_fn.cum_rate(2.0), 8.0, 0.0);
        // The cumulative rate for t=3.0 with an elapsed t=3.0 is still
        // only 2 days, since the infectiousness period ends at 5.0
        assert_almost_eq!(scaled_rate_fn.cum_rate(3.0), 8.0, 0.0);
    }
    #[test]
    fn test_scale_rate_fn_inverse_cum_rate() {
        let rate_fn = flat_capped();
        let scaled_rate_fn = TransformedInvertibleRateFn {
            base: &rate_fn,
            scale: 2.0,
            elapsed: 3.0,
        };
        assert_eq!(scaled_rate_fn.inverse_cum_rate(4.0), Some(1.0));
        assert_eq!(scaled_rate_fn.inverse_cum_rate(8.0), Some(2.0));
        assert_eq!(scaled_rate_fn.inverse_cum_rate(11.0), None);
    }
    #[test]
    fn test_scale_rate_fn_max_cum_rate() {
        let rate_fn = flat_capped();
        // Full curve area = 2.0 · 5.0 = 10.0.
        assert_almost_eq!(rate_fn.max_cum_rate(), 10.0, 0.0);
        // From elapsed 3.0 onward, the remaining area is 2.0 · 2.0 = 4.0, scaled by 2.0.
        let scaled_rate_fn = TransformedInvertibleRateFn {
            base: &rate_fn,
            scale: 2.0,
            elapsed: 3.0,
        };
        assert_almost_eq!(scaled_rate_fn.max_cum_rate(), 8.0, 0.0);
    }

    #[test]
    fn zero_scale_transform_is_unreachable() {
        // A zero contact bound (a size-1 household) must not invert to t = inf.
        let rate_fn = ConstantRateFn::new(1.0, 5.0);
        let zero = TransformedInvertibleRateFn::new(&rate_fn, 0.0, 0.0);
        assert_eq!(zero.inverse_cum_rate(1.0), None);
        assert_eq!(zero.inverse_cum_rate(0.0), Some(0.0));
        assert_eq!(zero.max_cum_rate(), 0.0);
        assert_eq!(zero.rate(3.0), 0.0);
        assert_eq!(zero.cum_rate(3.0), 0.0);

        let curve = flat_capped();
        let zero = TransformedInvertibleRateFn::new(&curve, 0.0, 1.0);
        assert_eq!(zero.inverse_cum_rate(0.5), None);
        assert_eq!(zero.max_cum_rate(), 0.0);
    }

    #[test]
    fn scaled_multiplies_the_rate_and_divides_the_inverse() {
        let rate_fn = flat_capped();
        let f = ScaledRateFn::new(TransformedInvertibleRateFn::new(&rate_fn, 2.0, 3.0), 3.0);
        assert_almost_eq!(f.rate(0.0), 12.0, 0.0);
        assert_almost_eq!(f.cum_rate(1.0), 12.0, 0.0);
        assert_eq!(f.inverse_cum_rate(12.0), Some(1.0));
        assert_eq!(f.max_cum_rate(), 24.0);
        let zero = ScaledRateFn::new(TransformedInvertibleRateFn::new(&rate_fn, 2.0, 3.0), 0.0);
        assert_eq!(zero.inverse_cum_rate(1.0), None);
        assert_eq!(zero.max_cum_rate(), 0.0);
    }
}

/// A rate of infection over time that can be integrated and inverted.
/// See `TransformedInvertibleRateFn` for an interval starting at a time other
/// than 0.
pub trait InvertibleRate {
    /// Returns the rate at time `t`
    fn rate(&self, t: f64) -> f64;

    /// Returns the expected number of events in the interval 0 -> t
    fn cum_rate(&self, t: f64) -> f64;

    /// Returns the time, starting at 0, by which `events` are expected to
    /// have occurred; `None` if [`cum_rate`](Self::cum_rate) never gets there.
    fn inverse_cum_rate(&self, events: f64) -> Option<f64>;
}

/// A rate function scaled by `scale` and shifted to start `elapsed` into `base`.
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

    /// The same function with its rate multiplied by `factor`.
    #[must_use]
    pub fn scaled(mut self, factor: f64) -> Self {
        self.scale *= factor;
        self
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
        let time = self
            .base
            .inverse_cum_rate(events / self.scale + elapsed_cum_rate)?;
        // Rounding can put `time` a float before `elapsed`.
        Some((time - self.elapsed).max(0.0))
    }
}

#[cfg(test)]
mod test {
    use super::{InvertibleRate, TransformedInvertibleRateFn};
    use crate::assert_almost_eq;
    use crate::core_epi::rate::{ConstantRateFn, EmpiricalRateFn};

    // Rate 2.0 on [0, 5], including the end, and 0 outside.
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
    fn inverse_is_never_before_now() {
        // Here `(0.3 * elapsed) / 0.3` rounds to just under `elapsed`.
        let rate_fn = ConstantRateFn::new(0.3, 1e9);
        let shifted = TransformedInvertibleRateFn::new(&rate_fn, 1.0, 1.685_000_000_000_002_3);
        assert_eq!(shifted.inverse_cum_rate(1e-300), Some(0.0));
    }

    #[test]
    fn zero_scale_transform_is_unreachable() {
        // A zero contact bound (a size-1 household) must not invert to t = inf.
        let rate_fn = ConstantRateFn::new(1.0, 5.0);
        let zero = TransformedInvertibleRateFn::new(&rate_fn, 0.0, 0.0);
        assert_eq!(zero.inverse_cum_rate(1.0), None);
        assert_eq!(zero.inverse_cum_rate(0.0), Some(0.0));
        assert_eq!(zero.rate(3.0), 0.0);
        assert_eq!(zero.cum_rate(3.0), 0.0);

        let curve = flat_capped();
        let zero = TransformedInvertibleRateFn::new(&curve, 0.0, 1.0);
        assert_eq!(zero.inverse_cum_rate(0.5), None);
    }

    #[test]
    fn scaled_multiplies_the_rate_and_divides_the_inverse() {
        let rate_fn = flat_capped();
        let f = TransformedInvertibleRateFn::new(&rate_fn, 2.0, 3.0).scaled(3.0);
        assert_almost_eq!(f.rate(0.0), 12.0, 0.0);
        assert_almost_eq!(f.cum_rate(1.0), 12.0, 0.0);
        assert_eq!(f.inverse_cum_rate(12.0), Some(1.0));
        assert_eq!(f.inverse_cum_rate(24.0 + 1e-9), None);
        let zero = TransformedInvertibleRateFn::new(&rate_fn, 2.0, 3.0).scaled(0.0);
        assert_eq!(zero.inverse_cum_rate(1.0), None);
    }
}

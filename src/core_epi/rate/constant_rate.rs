use super::InvertibleRate;

/// A constant infectiousness rate `value` over `[delay, delay + period)`, zero
/// before `delay` and from `delay + period` onwards.
///
/// [`ConstantRateFn::new`] sets `delay = 0.0`. Chain
/// [`with_delay`](Self::with_delay) to add a non-infectious latent period
/// without allocating a curve.
#[derive(Debug, Clone, Copy)]
pub struct ConstantRateFn {
    value: f64,
    delay: f64,
    infectious_period: f64,
}

impl ConstantRateFn {
    /// Creates a constant rate `value` over `[0, duration)`, zero afterwards.
    ///
    /// # Panics
    ///
    /// If `value` is negative or not finite, or `duration` is negative or NaN.
    #[must_use]
    pub fn new(value: f64, duration: f64) -> Self {
        assert!(
            value.is_finite() && value >= 0.0,
            "constant rate must be finite and non-negative, got {value}"
        );
        assert!(
            duration >= 0.0,
            "constant rate duration must be non-negative, got {duration}"
        );
        Self {
            value,
            delay: 0.0,
            infectious_period: duration,
        }
    }

    /// Sets a latent delay before infectiousness begins.
    ///
    /// The rate is `0.0` on `[0, delay)`, `value` on `[delay, delay + period)`,
    /// and `0.0` afterwards, where `period` is the duration passed to
    /// [`ConstantRateFn::new`]. Total [`duration`](Self::duration) becomes
    /// `delay + period`.
    ///
    /// # Panics
    ///
    /// If `delay` is negative or not finite.
    #[must_use]
    pub fn with_delay(mut self, delay: f64) -> Self {
        assert!(
            delay.is_finite() && delay >= 0.0,
            "constant rate delay must be finite and non-negative, got {delay}"
        );
        self.delay = delay;
        self
    }

    /// The latent delay before infectiousness begins (`0.0` unless set with
    /// [`with_delay`](Self::with_delay)).
    #[inline]
    #[must_use]
    pub fn delay(&self) -> f64 {
        self.delay
    }

    /// Time from `t = 0` until infectiousness ends (`delay` plus the
    /// infectious period passed to [`ConstantRateFn::new`]).
    #[inline]
    #[must_use]
    pub fn duration(&self) -> f64 {
        self.delay + self.infectious_period
    }
}

impl InvertibleRate for ConstantRateFn {
    #[inline]
    fn rate(&self, t: f64) -> f64 {
        if t >= self.delay && t < self.delay + self.infectious_period {
            self.value
        } else {
            0.0
        }
    }

    #[inline]
    fn cum_rate(&self, t: f64) -> f64 {
        if self.value == 0.0 || t <= self.delay {
            return 0.0;
        }
        self.value * (t - self.delay).clamp(0.0, self.infectious_period)
    }

    #[inline]
    fn inverse_cum_rate(&self, events: f64) -> Option<f64> {
        if events < 0.0 {
            return None;
        }
        if events == 0.0 {
            return Some(0.0);
        }
        if !events.is_finite() || self.value <= 0.0 || self.infectious_period <= 0.0 {
            return None;
        }
        let active_t = events / self.value;
        if !active_t.is_finite() || active_t > self.infectious_period {
            return None;
        }
        let t = self.delay + active_t;
        t.is_finite().then_some(t)
    }
}

#[cfg(test)]
mod test {
    use super::{ConstantRateFn, InvertibleRate};
    use crate::core_epi::rate::{EmpiricalRateFn, TransformedInvertibleRateFn};

    #[test]
    #[should_panic(expected = "duration must be non-negative")]
    fn a_negative_duration_panics() {
        let _ = ConstantRateFn::new(1.0, -1.0);
    }

    #[test]
    #[should_panic(expected = "duration must be non-negative")]
    fn a_nan_duration_panics() {
        let _ = ConstantRateFn::new(1.0, f64::NAN);
    }

    #[test]
    #[should_panic(expected = "rate must be finite and non-negative")]
    fn a_negative_rate_panics() {
        let _ = ConstantRateFn::new(-1.0, 1.0);
    }

    #[test]
    #[should_panic(expected = "delay must be finite and non-negative")]
    fn a_negative_delay_panics() {
        let _ = ConstantRateFn::new(1.0, 5.0).with_delay(-1.0);
    }

    #[test]
    #[should_panic(expected = "delay must be finite and non-negative")]
    fn an_infinite_delay_panics() {
        let _ = ConstantRateFn::new(1.0, 5.0).with_delay(f64::INFINITY);
    }

    #[test]
    #[should_panic(expected = "delay must be finite and non-negative")]
    fn a_nan_delay_panics() {
        let _ = ConstantRateFn::new(1.0, 5.0).with_delay(f64::NAN);
    }

    #[test]
    fn rate_is_flat_within_the_period() {
        let r = ConstantRateFn::new(0.7, 10.0);
        assert_eq!(r.delay(), 0.0);
        assert_eq!(r.duration(), 10.0);
        assert_eq!(r.rate(0.0), 0.7);
        assert_eq!(r.rate(9.99), 0.7);
        assert_eq!(r.rate(10.0), 0.0);
        assert_eq!(r.rate(-5.0), 0.0);
    }

    #[test]
    fn cum_rate_is_linear_then_flat() {
        let r = ConstantRateFn::new(0.5, 10.0);
        assert_eq!(r.cum_rate(0.0), 0.0);
        assert_eq!(r.cum_rate(10.0), 5.0);
        assert_eq!(r.cum_rate(100.0), 5.0);
        assert_eq!(r.cum_rate(-1.0), 0.0);
    }

    #[test]
    fn inverse_cum_rate_roundtrips() {
        let r = ConstantRateFn::new(0.3, 100.0);
        for c in [0.5, 1.0, 7.7] {
            let t = r.inverse_cum_rate(c).unwrap();
            assert!((r.cum_rate(t) - c).abs() < 1e-12);
        }
    }

    #[test]
    fn inverse_cum_rate_exhausts_at_the_period() {
        let r = ConstantRateFn::new(0.5, 10.0);
        assert_eq!(r.inverse_cum_rate(5.0), Some(10.0));
        assert_eq!(r.inverse_cum_rate(5.0 + 1e-9), None);
    }

    #[test]
    fn infinite_duration_is_unbounded() {
        let r = ConstantRateFn::new(0.5, f64::INFINITY);
        assert_eq!(r.inverse_cum_rate(1e9), Some(2e9));
    }

    #[test]
    fn inverse_cum_rate_zero_rate_is_unattainable() {
        let r = ConstantRateFn::new(0.0, 10.0);
        assert_eq!(r.inverse_cum_rate(1.0), None);
        // Zero events is always reachable at t=0.
        assert_eq!(r.inverse_cum_rate(0.0), Some(0.0));
    }

    #[test]
    fn inverse_cum_rate_infinite_events_is_unattainable() {
        // Never turn an infinite cumulative into an infinite plan time.
        let r = ConstantRateFn::new(0.5, f64::INFINITY);
        assert_eq!(r.inverse_cum_rate(f64::INFINITY), None);
        assert_eq!(r.inverse_cum_rate(f64::NAN), None);
    }

    #[test]
    fn overflow_underflow_and_zero_times_inf_are_safe() {
        let tiny_rate = ConstantRateFn::new(1e-300, f64::INFINITY);
        assert_eq!(tiny_rate.inverse_cum_rate(1e100), None);

        let zero_dur = ConstantRateFn::new(1e200, 0.0);
        assert_eq!(zero_dur.inverse_cum_rate(1e-200), None);

        let zero_rate = ConstantRateFn::new(0.0, f64::INFINITY);
        assert_eq!(zero_rate.cum_rate(f64::INFINITY), 0.0);

        let huge_delay = ConstantRateFn::new(1.0, 1e308).with_delay(1e308);
        assert_eq!(huge_delay.inverse_cum_rate(1e308), None);
    }

    #[test]
    fn delayed_rate_is_zero_before_delay_flat_during_period_and_zero_after() {
        let r = ConstantRateFn::new(0.7, 5.0).with_delay(3.0);
        assert_eq!(r.delay(), 3.0);
        assert_eq!(r.duration(), 8.0);
        assert_eq!(r.rate(-5.0), 0.0);
        assert_eq!(r.rate(0.0), 0.0);
        assert_eq!(r.rate(2.99), 0.0);
        assert_eq!(r.rate(3.0), 0.7);
        assert_eq!(r.rate(7.99), 0.7);
        assert_eq!(r.rate(8.0), 0.0);
        assert_eq!(r.rate(100.0), 0.0);
        assert_eq!(r.rate(f64::NAN), 0.0);
    }

    #[test]
    fn delayed_cum_rate_and_inverse_cum_rate() {
        let r = ConstantRateFn::new(0.5, 10.0).with_delay(3.0);
        assert_eq!(r.cum_rate(-1.0), 0.0);
        assert_eq!(r.cum_rate(0.0), 0.0);
        assert_eq!(r.cum_rate(3.0), 0.0);
        assert_eq!(r.cum_rate(5.0), 1.0);
        assert_eq!(r.cum_rate(13.0), 5.0);
        assert_eq!(r.cum_rate(100.0), 5.0);

        assert_eq!(r.inverse_cum_rate(0.0), Some(0.0));
        assert_eq!(r.inverse_cum_rate(1.0), Some(5.0));
        assert_eq!(r.inverse_cum_rate(5.0), Some(13.0));
        assert_eq!(r.inverse_cum_rate(5.0 + 1e-9), None);
        assert_eq!(r.inverse_cum_rate(-0.1), None);

        let unbounded = ConstantRateFn::new(0.5, f64::INFINITY).with_delay(3.0);
        assert_eq!(unbounded.duration(), f64::INFINITY);
        assert_eq!(unbounded.inverse_cum_rate(1e9), Some(3.0 + 2e9));
        assert_eq!(unbounded.inverse_cum_rate(f64::INFINITY), None);

        let zero_period = ConstantRateFn::new(2.0, 0.0).with_delay(3.0);
        assert_eq!(zero_period.rate(3.0), 0.0);
        assert_eq!(zero_period.cum_rate(10.0), 0.0);
        assert_eq!(zero_period.inverse_cum_rate(1e-12), None);
    }

    #[test]
    fn delayed_constant_matches_step_empirical_rate() {
        let latent = 2.5;
        let period = 6.0;
        let end = latent + period;
        let rate = 0.8;
        let delayed = ConstantRateFn::new(rate, period).with_delay(latent);
        let empirical = EmpiricalRateFn::new(vec![[latent, 0.0], [latent, rate], [end, rate]], 1.0);
        assert_eq!(delayed.duration(), empirical.duration());
        for t in [0.0, 1.0, 2.49, 2.5, 4.0, 8.49, 8.5, 10.0] {
            assert!((delayed.cum_rate(t) - empirical.cum_rate(t)).abs() < 1e-12);
        }
        for c in [0.0, 0.1, 1.5, 4.8] {
            let td = delayed.inverse_cum_rate(c).unwrap();
            let te = empirical.inverse_cum_rate(c).unwrap();
            assert!((td - te).abs() < 1e-12);
        }
        assert_eq!(delayed.inverse_cum_rate(4.81), None);
        assert_eq!(empirical.inverse_cum_rate(4.81), None);
    }

    #[test]
    fn delayed_transformed_during_and_after_delay() {
        let r = ConstantRateFn::new(0.5, 4.0).with_delay(3.0);
        // Still in latent period (elapsed = 1.0): 2.0 days left before infectiousness starts.
        let during_latent = TransformedInvertibleRateFn::new(&r, 2.0, 1.0);
        assert_eq!(during_latent.rate(0.0), 0.0);
        assert_eq!(during_latent.rate(1.99), 0.0);
        assert_eq!(during_latent.rate(2.0), 1.0);
        assert_eq!(during_latent.cum_rate(2.0), 0.0);
        assert_eq!(during_latent.cum_rate(3.0), 1.0);
        assert_eq!(during_latent.inverse_cum_rate(1.0), Some(3.0));
        assert_eq!(during_latent.inverse_cum_rate(4.0), Some(6.0));
        assert_eq!(during_latent.inverse_cum_rate(4.01), None);

        // Midway through infectious period (elapsed = 5.0): 2.0 days left.
        let during_inf = TransformedInvertibleRateFn::new(&r, 2.0, 5.0);
        assert_eq!(during_inf.rate(0.0), 1.0);
        assert_eq!(during_inf.cum_rate(2.0), 2.0);
        assert_eq!(during_inf.inverse_cum_rate(1.0), Some(1.0));
        assert_eq!(during_inf.inverse_cum_rate(2.01), None);

        // Past end (elapsed = 7.0).
        let past = TransformedInvertibleRateFn::new(&r, 2.0, 7.0);
        assert_eq!(past.inverse_cum_rate(1e-12), None);
    }

    #[test]
    fn with_delay_replaces_previous_delay() {
        let r = ConstantRateFn::new(0.5, 4.0)
            .with_delay(2.0)
            .with_delay(5.0);
        assert_eq!(r.delay(), 5.0);
        assert_eq!(r.duration(), 9.0);
        assert_eq!(r.rate(4.5), 0.0);
        assert_eq!(r.rate(5.0), 0.5);
    }
}

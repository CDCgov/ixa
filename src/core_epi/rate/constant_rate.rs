use super::InvertibleRate;

/// A constant infectiousness rate `value` over `[0, duration)`, zero afterwards.
///
/// `duration` is typically one person's own infectious period (drawn by the
/// model at infection), so the cumulative is finite and `inverse_cum_rate`
/// returns `None` once it is exhausted.
#[derive(Debug, Clone, Copy)]
pub struct ConstantRateFn {
    value: f64,
    duration: f64,
}

impl ConstantRateFn {
    #[must_use]
    pub fn new(value: f64, duration: f64) -> Self {
        Self { value, duration }
    }

    /// The infectious period this rate is truncated at.
    #[inline]
    #[must_use]
    pub fn duration(&self) -> f64 {
        self.duration
    }
}

impl InvertibleRate for ConstantRateFn {
    #[inline]
    fn rate(&self, t: f64) -> f64 {
        if t >= 0.0 && t < self.duration {
            self.value
        } else {
            0.0
        }
    }

    #[inline]
    fn cum_rate(&self, t: f64) -> f64 {
        self.value * t.clamp(0.0, self.duration)
    }

    #[inline]
    fn inverse_cum_rate(&self, events: f64) -> Option<f64> {
        if events < 0.0 {
            return None;
        }
        if events == 0.0 {
            return Some(0.0);
        }
        if !events.is_finite() || !self.value.is_finite() || self.value <= 0.0 {
            return None;
        }
        let t = events / self.value;
        (t <= self.duration).then_some(t)
    }

    #[inline]
    fn max_cum_rate(&self) -> f64 {
        if self.value <= 0.0 {
            0.0
        } else {
            self.value * self.duration
        }
    }
}

#[cfg(test)]
mod test {
    use super::{ConstantRateFn, InvertibleRate};

    #[test]
    fn rate_is_flat_within_the_period() {
        let r = ConstantRateFn::new(0.7, 10.0);
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
        assert_eq!(r.max_cum_rate(), 5.0);
        assert_eq!(r.inverse_cum_rate(5.0), Some(10.0));
        assert_eq!(r.inverse_cum_rate(5.0 + 1e-9), None);
    }

    #[test]
    fn infinite_duration_is_unbounded() {
        let r = ConstantRateFn::new(0.5, f64::INFINITY);
        assert_eq!(r.max_cum_rate(), f64::INFINITY);
        assert_eq!(r.inverse_cum_rate(1e9), Some(2e9));
    }

    #[test]
    fn inverse_cum_rate_zero_rate_is_unattainable() {
        let r = ConstantRateFn::new(0.0, 10.0);
        assert_eq!(r.inverse_cum_rate(1.0), None);
        // Zero events is always reachable at t=0.
        assert_eq!(r.inverse_cum_rate(0.0), Some(0.0));
        assert_eq!(r.max_cum_rate(), 0.0);
    }

    #[test]
    fn inverse_cum_rate_infinite_events_is_unattainable() {
        // Never turn an infinite cumulative into an infinite plan time.
        let r = ConstantRateFn::new(0.5, f64::INFINITY);
        assert_eq!(r.inverse_cum_rate(f64::INFINITY), None);
        assert_eq!(r.inverse_cum_rate(f64::NAN), None);
    }
}

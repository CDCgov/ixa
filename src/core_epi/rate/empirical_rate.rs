use super::InvertibleRate;

/// Linear interpolation of the curve at `t`; zero outside the anchor range.
#[inline]
fn rate_at_curve(points: &[[f64; 2]], t: f64) -> f64 {
    let (Some(first), Some(last)) = (points.first(), points.last()) else {
        return 0.0;
    };
    // Written so that a NaN `t` is outside the range too.
    if !(first[0]..=last[0]).contains(&t) {
        return 0.0;
    }
    // At least 1, since the first anchor is at or before `t`.
    let idx = points.partition_point(|p| p[0] <= t);
    if idx >= points.len() {
        return last[1];
    }
    let lo = points[idx - 1];
    let hi = points[idx];
    let alpha = (t - lo[0]) / (hi[0] - lo[0]);
    lo[1] + alpha * (hi[1] - lo[1])
}

/// Running trapezoidal integral at each anchor, starting at 0.
fn build_cum(points: &[[f64; 2]]) -> Vec<f64> {
    let mut cum = Vec::with_capacity(points.len());
    cum.push(0.0);
    let mut acc = 0.0;
    for i in 1..points.len() {
        let [ti, ri] = points[i - 1];
        let [tj, rj] = points[i];
        acc += 0.5 * (ri + rj) * (tj - ti);
        cum.push(acc);
    }
    cum
}

/// A piecewise-linear infectiousness curve through `[time, rate]` points,
/// times `scale`. The points are relative rates, so with an area-normalized
/// shape `scale` is the expected number of attempts.
#[derive(Clone, Debug)]
pub struct EmpiricalRateFn {
    points: Vec<[f64; 2]>,
    /// Integral of the unscaled shape up to each point.
    cum: Vec<f64>,
    scale: f64,
    duration: f64,
}

impl EmpiricalRateFn {
    /// Two points at one time make a step.
    ///
    /// # Panics
    ///
    /// If the times decrease, or a time, a rate or `scale` is negative or not
    /// finite.
    #[must_use]
    pub fn new(points: Vec<[f64; 2]>, scale: f64) -> Self {
        assert!(
            scale.is_finite() && scale >= 0.0,
            "empirical rate scale must be finite and non-negative, got {scale}"
        );
        let mut previous_time = 0.0;
        for &[time, rate] in &points {
            assert!(
                time.is_finite() && time >= previous_time,
                "empirical rate times must not decrease or be negative, got {time} after \
                 {previous_time}"
            );
            assert!(
                rate.is_finite() && rate >= 0.0,
                "empirical rates must be finite and non-negative, got {rate} at time {time}"
            );
            previous_time = time;
        }
        let cum = build_cum(&points);
        let total = *cum.last().unwrap();
        assert!(
            total.is_finite(),
            "empirical rate cumulative area must be finite, got {total}"
        );
        // The earliest anchor at which the cumulative peaks, so a trailing
        // tail of zeros does not count. An all-zero curve has duration 0.
        let duration = if total == 0.0 {
            0.0
        } else {
            cum.iter()
                .position(|&c| c == total)
                .and_then(|index| points.get(index))
                .map_or(0.0, |point| point[0])
        };
        Self {
            points,
            cum,
            scale,
            duration,
        }
    }

    /// The infectious period: the earliest time at which the cumulative
    /// infectiousness reaches its maximum.
    #[inline]
    #[must_use]
    pub fn duration(&self) -> f64 {
        self.duration
    }

    /// Integral of the unscaled shape up to `t`.
    #[inline]
    fn raw_cum(&self, t: f64) -> f64 {
        if t.is_nan() {
            return f64::NAN;
        }
        let points = &self.points;
        if t <= 0.0 || points.is_empty() {
            return 0.0;
        }
        if t <= points[0][0] {
            return 0.0;
        }
        for i in 0..points.len() - 1 {
            let [ti, ri] = points[i];
            let [tj, rj] = points[i + 1];
            if t < tj {
                let dt = t - ti;
                let slope = (rj - ri) / (tj - ti);
                let r_at_t = ri + slope * dt;
                return self.cum[i] + 0.5 * (ri + r_at_t) * dt;
            }
        }
        *self.cum.last().unwrap()
    }

    /// Solves `raw_cum(t) = c` for `c > 0.0`.
    #[inline]
    fn raw_inverse(&self, c: f64) -> Option<f64> {
        let points = &self.points;
        if points.is_empty() {
            return None;
        }
        for i in 0..points.len() - 1 {
            if c > self.cum[i + 1] {
                continue;
            }
            let [ti, ri] = points[i];
            let [tj, rj] = points[i + 1];
            let extra = c - self.cum[i];
            let span = tj - ti;
            let slope = (rj - ri) / span;
            // The root in rationalized form: `(-ri + sqrt(disc)) / slope`
            // cancels on a segment that is nearly flat.
            let disc = (ri * ri + 2.0 * slope * extra).max(0.0);
            let denom = ri + disc.sqrt();
            let u = if denom > 0.0 {
                (2.0 * extra / denom).min(span)
            } else {
                0.0
            };
            return Some(ti + u);
        }
        None
    }
}

impl InvertibleRate for EmpiricalRateFn {
    #[inline]
    fn rate(&self, t: f64) -> f64 {
        self.scale * rate_at_curve(&self.points, t)
    }

    #[inline]
    fn cum_rate(&self, t: f64) -> f64 {
        if self.scale == 0.0 {
            return 0.0;
        }
        self.scale * self.raw_cum(t)
    }

    /// `None` for a negative or NaN `c`, a zero `scale`, or a `c` beyond the
    /// curve's total area.
    #[inline]
    fn inverse_cum_rate(&self, c: f64) -> Option<f64> {
        if c.is_nan() || c < 0.0 {
            return None;
        }
        if c == 0.0 {
            return Some(0.0);
        }
        if !c.is_finite() || self.scale <= 0.0 {
            return None;
        }
        let target = c / self.scale;
        if target == 0.0 {
            // `c / scale` underflowed: return the start of the first positive-area
            // segment, or `None` if the curve has zero total area.
            for i in 0..self.points.len().saturating_sub(1) {
                if self.cum[i + 1] > 0.0 {
                    return Some(self.points[i][0]);
                }
            }
            return None;
        }
        self.raw_inverse(target)
    }
}

#[cfg(test)]
mod test {
    use super::{rate_at_curve, EmpiricalRateFn, InvertibleRate};

    fn unit(points: Vec<[f64; 2]>) -> EmpiricalRateFn {
        EmpiricalRateFn::new(points, 1.0)
    }

    #[test]
    fn rate_interpolates_and_is_zero_outside() {
        let r = unit(vec![[0.0, 0.2], [10.0, 0.8], [20.0, 0.4]]);
        assert_eq!(r.rate(-5.0), 0.0);
        assert_eq!(r.rate(100.0), 0.0);
        assert_eq!(r.rate(0.0), 0.2);
        assert_eq!(r.rate(10.0), 0.8);
        assert_eq!(r.rate(20.0), 0.4);
        assert!((r.rate(5.0) - 0.5).abs() < 1e-12);
        assert!((r.rate(15.0) - 0.6).abs() < 1e-12);
    }

    #[test]
    fn cum_rate_trapezoidal() {
        // The rate is t/10 on [0, 10], then 0.
        let r = unit(vec![[0.0, 0.0], [10.0, 1.0]]);
        assert!((r.cum_rate(5.0) - 1.25).abs() < 1e-12);
        assert!((r.cum_rate(10.0) - 5.0).abs() < 1e-12);
        assert!((r.cum_rate(15.0) - 5.0).abs() < 1e-12);
    }

    #[test]
    fn cum_rate_peak_shape_partial_segments() {
        let r = unit(vec![
            [0.0, 0.0],
            [1.0, 1.0],
            [2.0, 2.0],
            [3.0, 1.0],
            [4.0, 0.0],
        ]);
        assert!((r.cum_rate(1.5) - 1.125).abs() < 1e-12);
        assert!((r.cum_rate(2.5) - 2.875).abs() < 1e-12);
    }

    #[test]
    fn inverse_cum_rate_roundtrips_linear() {
        let r = unit(vec![[0.0, 0.0], [10.0, 2.0]]);
        for c in [0.1, 1.0, 5.0, 9.0] {
            let t = r.inverse_cum_rate(c).unwrap();
            assert!((r.cum_rate(t) - c).abs() < 1e-9, "c={c} t={t}");
        }
    }

    #[test]
    fn inverse_cum_rate_walks_plateau() {
        let r = unit(vec![
            [0.0, 1.0],
            [1.0, 1.0],
            [2.0, 0.0],
            [3.0, 0.0],
            [4.0, 1.0],
        ]);
        assert!((r.cum_rate(2.0) - 1.5).abs() < 1e-12);
        assert!((r.cum_rate(3.0) - 1.5).abs() < 1e-12);
        let t = r.inverse_cum_rate(1.5).unwrap();
        assert!((t - 2.0).abs() < 1e-12);
        let above = r.inverse_cum_rate(1.6).unwrap();
        assert!(above > 3.0, "expected past plateau, got {above}");
    }

    #[test]
    fn inverse_cum_rate_none_past_total() {
        let r = unit(vec![[0.0, 1.0], [5.0, 0.0]]);
        // Total area = 2.5; beyond that is unreachable.
        assert_eq!(r.inverse_cum_rate(2.5 + 1e-9), None);
        assert!(r.inverse_cum_rate(1.0).is_some());
    }

    #[test]
    fn inverse_cum_rate_of_a_non_finite_count_is_unattainable() {
        let r = unit(vec![[0.0, 1.0], [5.0, 1.0]]);
        assert_eq!(r.inverse_cum_rate(f64::NAN), None);
        assert_eq!(r.inverse_cum_rate(f64::INFINITY), None);
    }

    #[test]
    fn scale_multiplies_cum_rate() {
        let r = EmpiricalRateFn::new(vec![[0.0, 1.0], [10.0, 1.0]], 0.5);
        assert!((r.cum_rate(5.0) - 2.5).abs() < 1e-12);
        let t = r.inverse_cum_rate(2.5).unwrap();
        assert!((t - 5.0).abs() < 1e-9);
    }

    #[test]
    fn scale_zero_makes_curve_unreachable() {
        let r = EmpiricalRateFn::new(vec![[0.0, 1.0], [10.0, 1.0]], 0.0);
        assert_eq!(r.cum_rate(5.0), 0.0);
        assert_eq!(r.inverse_cum_rate(0.1), None);
        assert_eq!(r.inverse_cum_rate(0.0), Some(0.0));
    }

    #[test]
    fn duration_is_the_last_anchor_of_a_positive_curve() {
        assert_eq!(unit(vec![[0.0, 1.0], [5.0, 1.0]]).duration(), 5.0);
    }

    #[test]
    fn duration_ignores_a_trailing_zero_tail() {
        // The rate reaches 0 at 5 and is padded to 10.
        let r = unit(vec![[0.0, 1.0], [5.0, 0.0], [10.0, 0.0]]);
        assert_eq!(r.duration(), 5.0);
    }

    #[test]
    fn duration_of_an_all_zero_or_empty_curve_is_its_start() {
        assert_eq!(unit(vec![[0.0, 0.0], [10.0, 0.0]]).duration(), 0.0);
        assert_eq!(unit(vec![]).duration(), 0.0);
    }

    #[test]
    #[should_panic(expected = "times must not decrease")]
    fn unsorted_points_panic() {
        let _ = unit(vec![[2.0, 1.0], [0.0, 1.0]]);
    }

    #[test]
    #[should_panic(expected = "finite and non-negative")]
    fn a_negative_rate_panics() {
        let _ = unit(vec![[0.0, 1.0], [2.0, -3.0]]);
    }

    #[test]
    #[should_panic(expected = "scale must be finite and non-negative")]
    fn a_negative_scale_panics() {
        let _ = EmpiricalRateFn::new(vec![[0.0, 1.0], [2.0, 1.0]], -1.0);
    }

    #[test]
    fn inverse_cum_rate_is_accurate_on_a_nearly_flat_segment() {
        // Two rates that differ by rounding only, rising and falling.
        for (start, end) in [(0.3, 0.1 + 0.2), (0.1 + 0.2, 0.3), (1.0, 1.0 + 1e-13)] {
            let r = unit(vec![[0.0, start], [7.0, end]]);
            for t in [1.0, 3.0, 5.0, 7.0] {
                let inverted = r.inverse_cum_rate(start * t).unwrap();
                assert!(
                    (inverted - t).abs() < 1e-9,
                    "{start} to {end}: {inverted} for {t}"
                );
            }
        }
    }

    #[test]
    fn a_step_is_two_points_at_one_time() {
        let r = unit(vec![[0.0, 1.0], [2.0, 1.0], [2.0, 3.0], [4.0, 3.0]]);
        assert_eq!(r.rate(1.0), 1.0);
        assert_eq!(r.rate(2.0), 3.0);
        assert_eq!(r.cum_rate(3.0), 5.0);
        assert_eq!(r.inverse_cum_rate(5.0), Some(3.0));
    }

    #[test]
    fn cum_rate_zero_below_first_anchor() {
        let r = unit(vec![[1.0, 1.0], [2.0, 3.0], [3.0, 5.0]]);
        assert_eq!(r.cum_rate(0.0), 0.0);
        assert_eq!(r.cum_rate(0.5), 0.0);
    }

    #[test]
    fn rate_at_curve_outside_range_is_zero() {
        let pts = [[1.0, 1.0], [2.0, 3.0]];
        assert_eq!(rate_at_curve(&pts, 0.0), 0.0);
        assert_eq!(rate_at_curve(&pts, 3.0), 0.0);
        assert_eq!(rate_at_curve(&pts, 1.5), 2.0);
        assert_eq!(rate_at_curve(&[], 1.0), 0.0);
        assert_eq!(rate_at_curve(&pts, f64::NAN), 0.0);
    }

    #[test]
    fn nan_underflow_and_zero_area_edge_cases() {
        let delayed = EmpiricalRateFn::new(vec![[5.0, 1.0], [10.0, 1.0]], 1e200);
        assert!(delayed.cum_rate(f64::NAN).is_nan());
        assert_eq!(delayed.inverse_cum_rate(1e-200), Some(5.0));

        let all_zero = EmpiricalRateFn::new(vec![[5.0, 0.0], [10.0, 0.0]], 1e200);
        assert_eq!(all_zero.duration(), 0.0);
        assert_eq!(all_zero.inverse_cum_rate(1e-200), None);

        let empty = EmpiricalRateFn::new(vec![], 1e200);
        assert_eq!(empty.inverse_cum_rate(1e-200), None);

        let ramp = EmpiricalRateFn::new(vec![[0.0, 0.0], [10.0, 1.0]], 1.0);
        assert_eq!(ramp.inverse_cum_rate(1e-323), Some(0.0));
    }
}

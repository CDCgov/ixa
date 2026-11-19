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
    #[must_use]
    pub fn new(points: Vec<[f64; 2]>, scale: f64) -> Self {
        let cum = build_cum(&points);
        let total = *cum.last().unwrap();
        // The earliest anchor at which the cumulative peaks, so a trailing
        // tail of zeros does not count.
        let duration = cum
            .iter()
            .position(|&c| c == total)
            .and_then(|index| points.get(index))
            .map_or(0.0, |point| point[0]);
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

    /// Solves `raw_cum(t) = c`.
    #[inline]
    fn raw_inverse(&self, c: f64) -> Option<f64> {
        if c < 0.0 {
            return None;
        }
        // Reachable: a tiny `c` over a large scale underflows to 0.
        if c == 0.0 {
            return Some(0.0);
        }
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
            if slope == 0.0 {
                return Some(ti + extra / ri);
            }
            let disc = (ri * ri + 2.0 * slope * extra).max(0.0);
            let u = (-ri + disc.sqrt()) / slope;
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
        self.scale * self.raw_cum(t)
    }

    /// `None` for a negative `c`, a non-positive or non-finite `scale`, or a
    /// `c` beyond the curve's total area.
    #[inline]
    fn inverse_cum_rate(&self, c: f64) -> Option<f64> {
        if c < 0.0 {
            return None;
        }
        if c == 0.0 {
            return Some(0.0);
        }
        if !self.scale.is_finite() || self.scale <= 0.0 {
            return None;
        }
        self.raw_inverse(c / self.scale)
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
}

use super::InvertibleRate;

/// Cumulative hazard `Λ(τ)` for a single piecewise-linear empirical
/// curve. Slice-based helper (no precomputed cumulative): takes a slice so
/// the caller can avoid allocating or cloning. Used for area normalization,
/// benchmarks, and as the reference implementation `EmpiricalRateFn`'s
/// precomputed walk is pinned against.
#[inline]
pub fn empirical_cum_rate(points: &[[f64; 2]], t: f64) -> f64 {
    if t <= 0.0 || points.is_empty() {
        return 0.0;
    }
    let t0 = points[0][0];
    if t <= t0 {
        return 0.0;
    }
    let mut acc = 0.0;
    for i in 0..points.len() - 1 {
        let [ti, ri] = points[i];
        let [tj, rj] = points[i + 1];
        if t >= tj {
            acc += 0.5 * (ri + rj) * (tj - ti);
        } else {
            let dt = t - ti;
            let slope = (rj - ri) / (tj - ti);
            let r_at_t = ri + slope * dt;
            return acc + 0.5 * (ri + r_at_t) * dt;
        }
    }
    acc
}

/// Inverse cumulative hazard for a single piecewise-linear empirical
/// curve. `None` when `c` exceeds the curve's total integrated hazard.
/// Slice-based reference helper (see `empirical_cum_rate`).
#[inline]
pub fn empirical_inverse_cum_rate(points: &[[f64; 2]], c: f64) -> Option<f64> {
    if c < 0.0 {
        return None;
    }
    if c == 0.0 {
        return Some(0.0);
    }
    if points.is_empty() {
        return None;
    }
    let mut acc = 0.0;
    for i in 0..points.len() - 1 {
        let [ti, ri] = points[i];
        let [tj, rj] = points[i + 1];
        let seg = 0.5 * (ri + rj) * (tj - ti);
        if c > acc + seg {
            acc += seg;
            continue;
        }
        let extra = c - acc;
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

/// Last anchor time of a piecewise-linear curve — the upper end of its
/// support. Used to integrate the whole curve (`cum_rate` at
/// or past this time is the total area). `0.0` if the curve is empty.
///
/// Note: this is the curve's support boundary, **not** the recovery time —
/// see [`empirical_support_end`] for the latter (they differ for trailing-zero
/// tails).
#[inline]
pub fn empirical_curve_duration(points: &[[f64; 2]]) -> f64 {
    points.last().map_or(0.0, |p| p[0])
}

/// Recovery time for an empirical curve: the earliest τ at which the
/// cumulative infectiousness reaches its maximum (after which
/// `inverse_cum_rate` returns `None` and no further events occur).
///
/// - rate positive up to the last anchor → the last anchor (same as
///   [`empirical_curve_duration`]),
/// - trailing-zero tail → the end of the last positive-area segment,
/// - all-zero curve → the first anchor.
///
/// Independent of any `scale` (a positive scale doesn't move where the
/// cumulative peaks), so it's computed from the raw shape.
#[must_use]
pub fn empirical_support_end(points: &[[f64; 2]]) -> f64 {
    support_end_from_cum(points, &build_cum(points))
}

/// Recovery time from precomputed cumulative — the earliest anchor whose
/// cumulative equals the curve's max (factored out so `EmpiricalRateFn::new`
/// reuses the `cum` it already built rather than rebuilding it).
fn support_end_from_cum(points: &[[f64; 2]], cum: &[f64]) -> f64 {
    if points.is_empty() {
        return 0.0;
    }
    let max = *cum.last().unwrap();
    for (i, &c) in cum.iter().enumerate() {
        if c == max {
            return points[i][0];
        }
    }
    points[points.len() - 1][0]
}

/// Linear interpolation of a piecewise-linear curve at τ. Zero outside
/// the anchor range. Backs the per-event `rate` evaluation.
#[inline]
fn rate_at_curve(points: &[[f64; 2]], t: f64) -> f64 {
    if points.is_empty() {
        return 0.0;
    }
    let first = points[0];
    let last = points[points.len() - 1];
    if t < first[0] || t > last[0] {
        return 0.0;
    }
    let idx = points.partition_point(|p| p[0] <= t);
    if idx == 0 {
        return first[1];
    }
    if idx >= points.len() {
        return last[1];
    }
    let lo = points[idx - 1];
    let hi = points[idx];
    let span = hi[0] - lo[0];
    if span == 0.0 {
        return hi[1];
    }
    let alpha = (t - lo[0]) / span;
    lo[1] + alpha * (hi[1] - lo[1])
}

/// Running trapezoidal integral at each anchor (`cum[i] = Λ(points[i].0)`,
/// `cum[0] = 0`).
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

/// A per-person empirical infectiousness rate function: a piecewise-linear
/// curve λ(τ) with precomputed cumulative hazard at each anchor, plus a
/// `scale` multiplier applied at evaluation time. The point values are
/// **relative hazards**; `scale` converts them to absolute rates (so
/// `scale` is the expected R₀ once the shape is area-normalized).
///
/// The cumulative is built once at construction so the hot path skips the
/// per-segment trapezoid accumulation the slice helpers do every call.
/// `scale` is applied at evaluation (`scale · raw`), never baked into the
/// points, so the float arithmetic is identical to evaluating the shape and
/// scaling afterward.
#[derive(Clone, Debug)]
pub struct EmpiricalRateFn {
    points: Vec<[f64; 2]>,
    /// `cum[i] = Λ(points[i].0)` for the **unscaled** shape; `cum[0] = 0`.
    cum: Vec<f64>,
    scale: f64,
    duration: f64,
}

impl EmpiricalRateFn {
    #[must_use]
    pub fn new(points: Vec<[f64; 2]>, scale: f64) -> Self {
        let cum = build_cum(&points);
        let duration = support_end_from_cum(&points, &cum);
        Self {
            points,
            cum,
            scale,
            duration,
        }
    }

    /// The infectious period: earliest τ where cumulative infectiousness
    /// peaks. See [`empirical_support_end`].
    #[inline]
    #[must_use]
    pub fn duration(&self) -> f64 {
        self.duration
    }

    /// `Λ(τ)` for the **unscaled** shape. Walks segments to find the one
    /// containing `t`, then adds one partial trapezoid using the
    /// precomputed per-anchor cumulative.
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

    /// Solves `raw_cum(τ) = c` for the **unscaled** shape.
    #[inline]
    fn raw_inverse(&self, c: f64) -> Option<f64> {
        if c < 0.0 {
            return None;
        }
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
    /// Instantaneous rate `λ(τ) = scale · shape(τ)`; zero outside the
    /// curve's anchor range.
    #[inline]
    fn rate(&self, t: f64) -> f64 {
        self.scale * rate_at_curve(&self.points, t)
    }

    /// `Λ(τ) = scale · ∫₀^τ shape`.
    #[inline]
    fn cum_rate(&self, t: f64) -> f64 {
        self.scale * self.raw_cum(t)
    }

    /// Solves `cum_rate(τ) = c`. `None` for `c < 0`, for a non-positive or
    /// non-finite `scale`, or when `c` exceeds the curve's total integrated
    /// hazard.
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

    /// Total integrated hazard `scale · Λ(τ_end)`.
    #[inline]
    fn max_cum_rate(&self) -> f64 {
        self.scale * self.cum.last().copied().unwrap_or(0.0)
    }
}

#[cfg(test)]
mod test {
    use super::{
        empirical_cum_rate, empirical_curve_duration, empirical_inverse_cum_rate,
        empirical_support_end, rate_at_curve, EmpiricalRateFn, InvertibleRate,
    };

    fn unit(points: Vec<[f64; 2]>) -> EmpiricalRateFn {
        EmpiricalRateFn::new(points, 1.0)
    }

    // --- Instantaneous rate ------------------------------------------------

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

    // --- Cumulative + inverse ----------------------------------------------

    #[test]
    fn cum_rate_trapezoidal() {
        // λ(τ) = τ/10 on [0, 10], then 0.
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
        assert_eq!(r.inverse_cum_rate(3.0), None);
        assert!(r.inverse_cum_rate(1.0).is_some());
    }

    // --- Scale -------------------------------------------------------------

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
        assert_eq!(r.max_cum_rate(), 0.0);
    }

    // --- max_cum_rate ------------------------------------------------------

    #[test]
    fn max_cum_rate_is_total_area_times_scale() {
        let r = EmpiricalRateFn::new(vec![[0.0, 0.0], [1.0, 1.0], [2.0, 2.0]], 3.0);
        // Raw total area = 0.5 + 1.5 = 2.0; scaled = 6.0.
        assert!((r.max_cum_rate() - 6.0).abs() < 1e-12);
        assert_eq!(r.inverse_cum_rate(r.max_cum_rate() + 0.1), None);
    }

    // --- recovery time / support end --------------------------------------

    #[test]
    fn support_end_positive_to_last_anchor() {
        let r = unit(vec![[0.0, 1.0], [5.0, 1.0]]);
        assert_eq!(r.duration(), 5.0);
        assert_eq!(empirical_support_end(&[[0.0, 1.0], [5.0, 1.0]]), 5.0);
    }

    #[test]
    fn support_end_trailing_zero_tail() {
        // Rate hits 0 at τ=5, padding to τ=10 — recovery is at 5, not 10.
        let pts = vec![[0.0, 1.0], [5.0, 0.0], [10.0, 0.0]];
        assert_eq!(empirical_support_end(&pts), 5.0);
        assert_eq!(empirical_curve_duration(&pts), 10.0);
    }

    #[test]
    fn support_end_all_zero_is_first_anchor() {
        assert_eq!(empirical_support_end(&[[0.0, 0.0], [10.0, 0.0]]), 0.0);
    }

    // --- precomputed walk matches the slice helpers (bit-for-bit) ----------

    #[test]
    fn precomputed_matches_slice_helpers() {
        let points = vec![[0.0, 0.0], [1.0, 1.0], [2.0, 2.0], [3.0, 1.0], [4.0, 0.0]];
        let r = unit(points.clone());
        for t in [-0.1, 0.0, 0.5, 1.0, 1.5, 2.0, 2.5, 3.0, 3.7, 4.0, 5.0] {
            assert_eq!(r.cum_rate(t), empirical_cum_rate(&points, t), "t={t}");
        }
        for c in [-0.1, 0.0, 0.1, 0.5, 1.125, 2.0, 2.875, 3.5, 4.0, 5.0] {
            assert_eq!(
                r.inverse_cum_rate(c),
                empirical_inverse_cum_rate(&points, c),
                "c={c}"
            );
        }
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
    }
}

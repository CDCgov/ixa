use super::InvertibleRate;

/// From this many anchors on, lookups go through a [`Guide`]. Below it a
/// guide does not pay for `inverse_cum_rate`.
const GUIDE_MIN_LEN: usize = 16;

/// One cell per key is slower and four are no faster.
const GUIDE_CELLS_PER_KEY: usize = 2;

/// Narrows a binary search over sorted keys. The key range is cut into
/// uniform cells and `first[cell]` is the index of the first key in that cell
/// or a later one. Keys and lookups get their cell from the same monotone
/// arithmetic, so a key in an earlier cell than `x` is below `x` and a key in
/// a later one is above it, whatever the rounding: the partition point of `x`
/// is in `first[cell]..=first[cell + 1]`, for `<` and for `<=`.
///
/// A key far from the rest leaves the others in one cell, where the search
/// is as wide as without a guide.
#[derive(Clone, Debug)]
struct Guide {
    start: f64,
    cells_per_unit: f64,
    first: Box<[u32]>,
}

impl Guide {
    /// A guide over the non-decreasing `key(0..len)`, or `None` when the keys
    /// span nothing, or too little for a finite cell count per unit.
    fn new(len: usize, key: impl Fn(usize) -> f64) -> Option<Self> {
        if len < 2 || len > u32::MAX as usize {
            return None;
        }
        let start = key(0);
        let cells = GUIDE_CELLS_PER_KEY * len;
        let cells_per_unit = cells as f64 / (key(len - 1) - start);
        if !cells_per_unit.is_finite() {
            return None;
        }
        let mut guide = Self {
            start,
            cells_per_unit,
            first: vec![0; cells + 1].into_boxed_slice(),
        };
        // Count the keys of each cell one slot up, then accumulate.
        for index in 0..len {
            let cell = guide.cell(key(index));
            guide.first[cell + 1] += 1;
        }
        let mut keys_before = 0;
        for slot in &mut guide.first {
            keys_before += *slot;
            *slot = keys_before;
        }
        Some(guide)
    }

    /// Monotone in `x`. The cast sends a negative or NaN product to cell 0
    /// and saturates on infinity.
    #[inline]
    fn cell(&self, x: f64) -> usize {
        let cell = ((x - self.start) * self.cells_per_unit) as usize;
        cell.min(self.first.len() - 2)
    }

    /// Bounds on the partition point of `x`.
    #[inline]
    fn range(&self, x: f64) -> (usize, usize) {
        let cell = self.cell(x);
        (self.first[cell] as usize, self.first[cell + 1] as usize)
    }
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
    /// Over the anchor times. Boxed so a curve too short for one stays small.
    time_guide: Option<Box<Guide>>,
    /// Over `cum`.
    cum_guide: Option<Box<Guide>>,
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
        let mut time_guide = None;
        let mut cum_guide = None;
        if points.len() >= GUIDE_MIN_LEN {
            time_guide = Guide::new(points.len(), |index| points[index][0]).map(Box::new);
            cum_guide = Guide::new(cum.len(), |index| cum[index]).map(Box::new);
        }
        Self {
            points,
            cum,
            scale,
            duration,
            time_guide,
            cum_guide,
        }
    }

    /// The infectious period: the earliest time at which the cumulative
    /// infectiousness reaches its maximum.
    #[inline]
    #[must_use]
    pub fn duration(&self) -> f64 {
        self.duration
    }

    /// How many anchors are at or before `t`.
    #[inline]
    fn anchors_through(&self, t: f64) -> usize {
        let Some(guide) = &self.time_guide else {
            return self.points.partition_point(|p| p[0] <= t);
        };
        let (lo, hi) = guide.range(t);
        lo + self.points[lo..hi].partition_point(|p| p[0] <= t)
    }

    /// How many anchors have a cumulative below `c`.
    #[inline]
    fn anchors_below_cum(&self, c: f64) -> usize {
        let Some(guide) = &self.cum_guide else {
            return self.cum.partition_point(|&cum| cum < c);
        };
        let (lo, hi) = guide.range(c);
        lo + self.cum[lo..hi].partition_point(|&cum| cum < c)
    }

    /// The unscaled shape at `t`: linear between anchors, zero outside them.
    #[inline]
    fn raw_rate(&self, t: f64) -> f64 {
        let points = &self.points;
        let (Some(first), Some(last)) = (points.first(), points.last()) else {
            return 0.0;
        };
        // Written so that a NaN `t` is outside the range too.
        if !(first[0]..=last[0]).contains(&t) {
            return 0.0;
        }
        // At least 1, since the first anchor is at or before `t`.
        let idx = self.anchors_through(t);
        if idx >= points.len() {
            return last[1];
        }
        let lo = points[idx - 1];
        let hi = points[idx];
        let alpha = (t - lo[0]) / (hi[0] - lo[0]);
        lo[1] + alpha * (hi[1] - lo[1])
    }

    /// Integral of the unscaled shape up to `t`.
    #[inline]
    fn raw_cum(&self, t: f64) -> f64 {
        if t.is_nan() {
            return f64::NAN;
        }
        let points = &self.points;
        if t <= 0.0 || points.is_empty() || t <= points[0][0] {
            return 0.0;
        }
        let idx = self.anchors_through(t);
        if idx >= points.len() {
            return *self.cum.last().unwrap();
        }
        let i = idx - 1;
        let [ti, ri] = points[i];
        let [tj, rj] = points[idx];
        let dt = t - ti;
        let slope = (rj - ri) / (tj - ti);
        let r_at_t = ri + slope * dt;
        self.cum[i] + 0.5 * (ri + r_at_t) * dt
    }

    /// Solves `raw_cum(t) = c` for `c > 0.0`.
    #[inline]
    fn raw_inverse(&self, c: f64) -> Option<f64> {
        let points = &self.points;
        if points.is_empty() {
            return None;
        }
        let idx = self.anchors_below_cum(c);
        if idx >= points.len() {
            return None;
        }
        let i = idx - 1;
        let [ti, ri] = points[i];
        let [tj, rj] = points[idx];
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
        Some(ti + u)
    }
}

impl InvertibleRate for EmpiricalRateFn {
    #[inline]
    fn rate(&self, t: f64) -> f64 {
        self.scale * self.raw_rate(t)
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
    use rand::rngs::StdRng;
    use rand::{RngExt, SeedableRng};

    use super::{EmpiricalRateFn, Guide, InvertibleRate, GUIDE_MIN_LEN};

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
    fn rate_outside_range_is_zero() {
        let r = unit(vec![[1.0, 1.0], [2.0, 3.0]]);
        assert_eq!(r.rate(0.0), 0.0);
        assert_eq!(r.rate(3.0), 0.0);
        assert_eq!(r.rate(1.5), 2.0);
        assert_eq!(r.rate(f64::NAN), 0.0);
        assert_eq!(unit(vec![]).rate(1.0), 0.0);
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

    /// Each key, the floats beside it and the extremes.
    fn probes(keys: &[f64]) -> Vec<f64> {
        let mut probes = vec![0.0, -1.0, f64::NEG_INFINITY, f64::INFINITY, f64::NAN];
        for &key in keys {
            probes.extend([key.next_down(), key, key.next_up()]);
        }
        if let Some(&last) = keys.last() {
            probes.extend([last + 1.0, 2.0 * last]);
        }
        probes
    }

    /// The guide's range holds both partition points of every probe.
    fn assert_guide_brackets(keys: &[f64]) {
        let Some(guide) = Guide::new(keys.len(), |index| keys[index]) else {
            return;
        };
        assert_eq!(guide.first[0], 0);
        assert_eq!(*guide.first.last().unwrap() as usize, keys.len());
        for x in probes(keys) {
            if x.is_nan() {
                continue;
            }
            let (lo, hi) = guide.range(x);
            let below = keys.partition_point(|&key| key < x);
            let through = keys.partition_point(|&key| key <= x);
            assert!(
                lo <= below && through <= hi,
                "{x}: {below}..={through} is outside {lo}..={hi}"
            );
        }
    }

    /// The curve with a guide over each of its keys, whatever its length, and
    /// the same curve with none.
    fn guided_and_plain(points: Vec<[f64; 2]>, scale: f64) -> (EmpiricalRateFn, EmpiricalRateFn) {
        let mut guided = EmpiricalRateFn::new(points, scale);
        let len = guided.points.len();
        guided.time_guide = Guide::new(len, |index| guided.points[index][0]).map(Box::new);
        guided.cum_guide = Guide::new(len, |index| guided.cum[index]).map(Box::new);
        let mut plain = guided.clone();
        plain.time_guide = None;
        plain.cum_guide = None;
        (guided, plain)
    }

    /// Bit for bit, with every NaN taken as one value.
    fn bits(x: f64) -> u64 {
        if x.is_nan() {
            u64::MAX
        } else {
            x.to_bits()
        }
    }

    fn assert_same_results(guided: &EmpiricalRateFn, plain: &EmpiricalRateFn, rng: &mut StdRng) {
        let times: Vec<f64> = plain.points.iter().map(|point| point[0]).collect();
        let last_time = times.last().copied().unwrap_or(1.0);
        let mut time_probes = probes(&times);
        for _ in 0..100 {
            time_probes.push(rng.random_range(0.0..=1.1 * last_time + 1.0));
        }
        for t in time_probes {
            assert_eq!(bits(guided.rate(t)), bits(plain.rate(t)), "rate({t})");
            assert_eq!(
                bits(guided.cum_rate(t)),
                bits(plain.cum_rate(t)),
                "cum_rate({t})"
            );
        }

        let total = plain.cum.last().copied().unwrap_or(0.0);
        let mut raw_counts = probes(&plain.cum);
        for _ in 0..100 {
            raw_counts.push(rng.random_range(0.0..=1.05 * total + 1e-3));
        }
        for raw in raw_counts {
            if raw > 0.0 {
                assert_eq!(
                    guided.raw_inverse(raw).map(bits),
                    plain.raw_inverse(raw).map(bits),
                    "raw_inverse({raw})"
                );
            }
            for c in [raw, raw * plain.scale] {
                assert_eq!(
                    guided.inverse_cum_rate(c).map(bits),
                    plain.inverse_cum_rate(c).map(bits),
                    "inverse_cum_rate({c})"
                );
            }
        }
    }

    /// Uneven gaps (steps, hair-thin, ordinary, and long ones that put most
    /// anchors in a few cells) and rates with stretches of zeros.
    fn random_curve(rng: &mut StdRng) -> Vec<[f64; 2]> {
        let len = match rng.random_range(0..4) {
            0 => rng.random_range(0..4),
            1 => rng.random_range(4..40),
            2 => rng.random_range(40..200),
            _ => rng.random_range(200..800),
        };
        let all_zero = rng.random_bool(0.05);
        let zero_share = rng.random_range(0.0..0.6);
        let leading_zeros = rng.random_range(0..=len / 3);
        let trailing_zeros = rng.random_range(0..=len / 3);
        let long_gap_share = rng.random_range(0.0..0.05);
        let mut time = if rng.random_bool(0.5) {
            0.0
        } else {
            rng.random_range(0.0..1e3)
        };
        let mut in_zero_stretch = false;
        let mut points = Vec::with_capacity(len);
        for index in 0..len {
            if index > 0 {
                let kind: f64 = rng.random();
                time += if kind < 0.15 {
                    0.0
                } else if kind < 0.2 {
                    rng.random_range(0.0..1e-9)
                } else if kind < 0.2 + long_gap_share {
                    rng.random_range(100.0..1e5)
                } else {
                    rng.random_range(0.0..1.0)
                };
            }
            if rng.random_bool(0.2) {
                in_zero_stretch = rng.random_bool(zero_share);
            }
            let zero = all_zero
                || in_zero_stretch
                || index < leading_zeros
                || index + trailing_zeros >= len;
            let rate = if zero {
                0.0
            } else {
                rng.random_range(0.0..3.0)
            };
            points.push([time, rate]);
        }
        points
    }

    #[test]
    fn guided_lookups_equal_plain_binary_searches() {
        let mut rng = StdRng::seed_from_u64(20261123);
        for _ in 0..1500 {
            let points = random_curve(&mut rng);
            let scale = if rng.random_bool(0.3) {
                1.0
            } else {
                rng.random_range(0.01..50.0)
            };
            let (guided, plain) = guided_and_plain(points, scale);
            assert_guide_brackets(&plain.cum);
            let times: Vec<f64> = plain.points.iter().map(|point| point[0]).collect();
            assert_guide_brackets(&times);
            assert_same_results(&guided, &plain, &mut rng);
        }
    }

    #[test]
    fn a_curve_gets_guides_from_the_minimum_length_on() {
        let curve = |len: usize| unit((0..len).map(|index| [index as f64, 1.0]).collect());
        let short = curve(GUIDE_MIN_LEN - 1);
        assert!(short.time_guide.is_none() && short.cum_guide.is_none());
        let long = curve(GUIDE_MIN_LEN);
        assert!(long.time_guide.is_some() && long.cum_guide.is_some());
        // Nothing to guide over: every cumulative is 0, every time is the same.
        let zero_area = unit(vec![[1.0, 0.0]; GUIDE_MIN_LEN]);
        assert!(zero_area.time_guide.is_none() && zero_area.cum_guide.is_none());
    }

    #[test]
    fn guided_lookups_are_exact_at_both_ends() {
        // Steps of 0.1 and 0.7 do not land on cell edges, so the first and
        // last cells are where an edge computed apart from the keys would
        // round to the wrong side.
        for len in GUIDE_MIN_LEN..400 {
            for step in [0.1, 0.7] {
                let points: Vec<[f64; 2]> = (0..len)
                    .map(|index| [5.0 + step * index as f64, 1.0])
                    .collect();
                let last_time = points[len - 1][0];
                let r = unit(points);
                let total = *r.cum.last().unwrap();
                assert_eq!(r.cum_rate(f64::INFINITY), total);
                assert_eq!(r.cum_rate(last_time), total);
                assert_eq!(r.rate(last_time), 1.0);
                assert_eq!(r.rate(last_time.next_up()), 0.0);
                // Before the first anchor the partition point is 0.
                assert_eq!(r.cum_rate(5.0_f64.next_down()), 0.0);
                assert_eq!(r.rate(5.0_f64.next_down()), 0.0);
                assert_eq!(r.rate(5.0), 1.0);
                assert_eq!(r.inverse_cum_rate(total), Some(last_time));
                assert_eq!(r.inverse_cum_rate(total.next_up()), None);
            }
        }
    }

    #[test]
    fn a_guide_brackets_keys_its_cells_cannot_tell_apart() {
        // A span of a few floats far from zero: cells narrower than the
        // spacing of the floats themselves.
        let mut close = vec![1e9_f64; 40];
        for (index, key) in close.iter_mut().enumerate() {
            for _ in 0..index / 5 {
                *key = key.next_up();
            }
        }
        assert_guide_brackets(&close);

        // A span so small that the cells per unit overflow.
        let tiny: Vec<f64> = (0..40).map(|index| f64::from(index) * 5e-324).collect();
        assert!(Guide::new(tiny.len(), |index| tiny[index]).is_none());

        // One key far out leaves every other key in the first cell.
        let mut tail: Vec<f64> = (0..40).map(f64::from).collect();
        tail.push(1e12);
        assert_guide_brackets(&tail);

        // A flat run, as the cumulative of a zero-rate stretch is.
        let mut flat = vec![0.0, 0.5];
        flat.extend([1.0; 30]);
        flat.push(1.5);
        assert_guide_brackets(&flat);
    }
}

//! Algorithms for uniform random sampling from hash sets or iterators. These algorithms are written to be generic
//! over the container type using zero-cost trait abstractions.

use crate::rand::seq::index::sample as choose_range;
use crate::rand::Rng;

/// Sample a random element uniformly from a container of known length.
///
/// We do not assume the container is randomly indexable, only that it can be iterated over.
/// This algorithm is used when the property is indexed, and thus we know the length of the result set.
pub fn sample_single_from_known_length<'a, I, R, T>(rng: &mut R, mut iter: I) -> Option<T>
where
    R: Rng,
    I: Iterator<Item = T> + ExactSizeIterator<Item = T>,
{
    let len = iter.len();
    if len == 0 {
        return None;
    }
    // This little trick with `u32` makes this function 30% faster.
    let index = rng.random_range(0..len as u32) as usize;
    // The set need not be randomly indexable, so we have to use the `nth` method.
    iter.nth(index)
}

/// Sample a random element uniformly from an iterator of unknown length.
///
/// We do not assume the container is randomly indexable, only that it can be iterated over.
///
/// This function implements "Algorithm L" from KIM-HUNG LI
/// Reservoir-Sampling Algorithms of Time Complexity O(n(1 + log(N/n)))
/// <https://dl.acm.org/doi/pdf/10.1145/198429.198435>
///
/// This algorithm is significantly slower than the "known length" algorithm (factor
/// of 10^4). The reservoir algorithm from `rand` reduces to the "known length`
/// algorithm when the iterator is an `ExactSizeIterator`, or more precisely,
/// when `iterator.size_hint()` returns `(k, Some(k))` for some `k`. Otherwise,
/// this algorithm is much faster than the `rand` implementation (factor of 100).
pub fn sample_single_l_reservoir<I, R, T>(rng: &mut R, iterable: I) -> Option<T>
where
    R: Rng,
    I: IntoIterator<Item = T>,
{
    let mut chosen_item: Option<T> = None; // the currently selected element
    let mut weight: f64 = rng.random_range(0.0..1.0); // controls skip distance distribution
    let mut position: usize = 0; // current index in data
    let mut next_pick_position: usize = 1; // index of the next item to pick

    iterable.into_iter().for_each(|item| {
        position += 1;
        if position == next_pick_position {
            chosen_item = Some(item);
            next_pick_position +=
                (f64::ln(rng.random_range(0.0..1.0)) / f64::ln(1.0 - weight)).floor() as usize + 1;
            weight *= rng.random_range(0.0..1.0);
        }
    });

    chosen_item
}

/// Sample multiple random elements uniformly without replacement from a container of known length.
/// This function assumes `set.len() >= requested`.
///
/// We do not assume the container is randomly indexable, only that it can be iterated over. The values are cloned.
///
/// This algorithm can be used when the property is indexed, and thus we know the length of the result set.
/// For very small `requested` values (<=5), this algorithm is faster than reservoir because it doesn't
/// iterate over the entire set.
pub fn sample_multiple_from_known_length<'a, I, R, T>(
    rng: &mut R,
    iter: I,
    requested: usize,
) -> Vec<T>
where
    R: Rng,
    I: IntoIterator<Item = T> + ExactSizeIterator<Item = T>,
{
    let mut indexes = Vec::with_capacity(requested);
    indexes.extend(choose_range(rng, iter.len(), requested));
    indexes.sort_unstable();
    let mut index_iterator = indexes.into_iter();
    let mut next_idx = index_iterator.next().unwrap();
    let mut selected = Vec::with_capacity(requested);

    for (idx, item) in iter.enumerate() {
        if idx == next_idx {
            selected.push(item);
            if let Some(i) = index_iterator.next() {
                next_idx = i;
            } else {
                break;
            }
        }
    }

    selected
}

/// Sample multiple random elements uniformly without replacement from a container of unknown length. If
/// more samples are requested than are in the set, the function returns as many items as it can.
///
/// We do not assume the container is randomly indexable, only that it can be iterated over. The values are cloned.
///
/// This function implements "Algorithm L" from KIM-HUNG LI
/// Reservoir-Sampling Algorithms of Time Complexity O(n(1 + log(N/n)))
/// <https://dl.acm.org/doi/pdf/10.1145/198429.198435>
///
/// This algorithm is significantly faster than the reservoir algorithm in `rand` and is
/// on par with the "known length" algorithm for large `requested` values.
pub fn sample_multiple_l_reservoir<I, R, T>(rng: &mut R, iter: I, requested: usize) -> Vec<T>
where
    R: Rng,
    I: IntoIterator<Item = T>,
{
    let mut weight: f64 = rng.random_range(0.0..1.0); // controls skip distance distribution
    weight = weight.powf(1.0 / requested as f64);
    let mut position: usize = 0; // current index in data
    let mut next_pick_position: usize = 1; // index of the next item to pick
    let mut reservoir = Vec::with_capacity(requested); // the sample reservoir

    iter.into_iter().for_each(|item| {
        position += 1;
        if position == next_pick_position {
            if reservoir.len() == requested {
                let to_remove = rng.random_range(0..reservoir.len());
                reservoir.swap_remove(to_remove);
            }
            reservoir.push(item);

            if reservoir.len() == requested {
                next_pick_position += (f64::ln(rng.random_range(0.0..1.0)) / f64::ln(1.0 - weight))
                    .floor() as usize
                    + 1;
                let uniform_random: f64 = rng.random_range(0.0..1.0);
                weight *= uniform_random.powf(1.0 / requested as f64);
            } else {
                next_pick_position += 1;
            }
        }
    });

    reservoir
}

#[cfg(test)]
mod tests {
    use rand::rngs::StdRng;
    use rand::SeedableRng;

    use super::*;
    use crate::HashSet;
    #[test]
    fn test_sample_multiple_l_reservoir_basic() {
        let data: Vec<u32> = (0..1000).collect();
        let requested = 100;
        let seed: u64 = 42;
        let mut rng = StdRng::seed_from_u64(seed);
        let sample = sample_multiple_l_reservoir(&mut rng, &data, requested);

        // Correct sample size
        assert_eq!(sample.len(), requested);

        // All sampled values are within the valid range
        assert!(sample.iter().all(|v| **v < 1000));

        // The sample should not have duplicates
        let unique: HashSet<_> = sample.iter().collect();
        assert_eq!(unique.len(), sample.len());
    }

    // Verifies that the reservoir sampling algorithm produces uniformly distributed
    // samples by running it 1000 times and checking that the resulting chi-square
    // statistics follow the expected chi-square(9) distribution. Note that this
    // test is only approximately correct, reasonable only when `requested` is small
    // relative to `population`, because `sample_multiple_l_reservoir` samples
    // without replacement, while the chi-squared test assumes independent samples.
    #[test]
    fn test_sample_multiple_l_reservoir_uniformity() {
        let population: u32 = 10000;
        let data: Vec<u32> = (0..population).collect();
        let requested = 100;
        let num_runs = 1000;
        let mut chi_squares = Vec::with_capacity(num_runs);

        for run in 0..num_runs {
            let mut rng = StdRng::seed_from_u64(42 + run as u64);
            let sample = sample_multiple_l_reservoir(&mut rng, data.iter().copied(), requested);

            // Partition range 0..population into 10 equal-width bins
            let mut counts = [0usize; 10];
            for value in sample {
                let bin = (value as usize) / (population as usize / 10);
                counts[bin] += 1;
            }

            // Expected count per bin for uniform sampling
            let expected = requested as f64 / 10.0; // = 10.0

            // Compute chi-square statistic
            let chi_square: f64 = counts
                .iter()
                .map(|&obs| {
                    let diff = (obs as f64) - expected;
                    diff * diff / expected
                })
                .sum();

            chi_squares.push(chi_square);
        }

        // Now test that chi_squares follow a chi-square distribution with df=9
        // We use quantiles of the chi-square(9) distribution to create bins
        // and check if the observed counts match the expected uniform distribution

        // Quantiles of chi-square distribution with df=9 at deciles (10 bins)
        // These values define the bin boundaries such that each bin should contain
        // 10% of the observations if they truly follow chi-square(9).
        // Generate with Mathematica:
        //     Table[Quantile[ChiSquareDistribution[9], p/10], {p, 0, 10}]//N
        let quantiles = [
            0.0,           // 0th percentile (minimum)
            4.16816,       // 10th percentile
            5.38005,       // 20th percentile
            6.39331,       // 30th percentile
            7.35703,       // 40th percentile
            8.34283,       // 50th percentile (median)
            9.41364,       // 60th percentile
            10.6564,       // 70th percentile
            12.2421,       // 80th percentile
            14.6837,       // 90th percentile
            f64::INFINITY, // 100th percentile (maximum)
        ];

        let num_bins = quantiles.len() - 1;
        let mut chi_square_counts = vec![0usize; num_bins];

        for &chi_sq in &chi_squares {
            // Find which bin this chi-square value falls into
            for i in 0..num_bins {
                if chi_sq >= quantiles[i] && chi_sq < quantiles[i + 1] {
                    chi_square_counts[i] += 1;
                    break;
                }
            }
        }

        // Each bin should contain approximately num_runs / num_bins observations
        let expected_per_bin = num_runs as f64 / num_bins as f64;
        let chi_square_of_chi_squares: f64 = chi_square_counts
            .iter()
            .map(|&obs| {
                let diff = (obs as f64) - expected_per_bin;
                diff * diff / expected_per_bin
            })
            .sum();

        // Degrees of freedom = (#bins - 1) = 9
        // Critical χ²₀.₉₉₉ for df=9 is 27.877
        let critical = 27.877;

        println!(
            "χ² = {}, counts = {:?}",
            chi_square_of_chi_squares, chi_square_counts
        );

        assert!(
            chi_square_of_chi_squares < critical,
            "Chi-square statistics fail to follow chi-square(9) distribution: χ² = {}, counts = {:?}",
            chi_square_of_chi_squares,
            chi_square_counts
        );
    }
}

//! Algorithms for uniform random sampling from hash sets or iterators. These algorithms are written to be generic
//! over the container type using zero-cost trait abstractions.
use std::collections::{HashMap, HashSet};

use crate::rand::seq::index::sample as choose_range;
use crate::rand::Rng;

/// The `len` capability, a zero-cost abstraction for types that have a known length.
pub trait HasLen {
    fn len(&self) -> usize;
}

/// The `iter` capability, a zero-cost abstraction for types that can be iterated over.
pub trait HasIter {
    type Item<'a>
    where
        Self: 'a;
    type Iter<'a>: Iterator<Item = Self::Item<'a>>
    where
        Self: 'a;

    fn iter(&self) -> Self::Iter<'_>;
}

macro_rules! impl_has_len {
    ($ty:ident < $($gen:ident),* >) => {
        impl<$($gen),*> HasLen for $ty<$($gen),*> {
            fn len(&self) -> usize {
                <$ty<$($gen),*>>::len(self)
            }
        }
    };
}

macro_rules! impl_has_iter {
    ($ty:ident < $($gen:ident),* >, $iter:ty, $item:ty) => {
        impl<$($gen),*> HasIter for $ty<$($gen),*> {
            type Item<'a> = $item where Self: 'a;
            type Iter<'a> = $iter where Self: 'a;

            fn iter(&self) -> Self::Iter<'_> {
                <$ty<$($gen),*>>::iter(self)
            }
        }
    };
}

// Vec<T>
impl_has_len!(Vec<T>);
// We implement `HasIter` manually for `Vec<T>` because its `iter` method is from `Deref<Target = [T]>`.
impl<T> HasIter for Vec<T> {
    type Item<'a>
        = &'a T
    where
        Self: 'a;
    type Iter<'a>
        = std::slice::Iter<'a, T>
    where
        Self: 'a;

    fn iter(&self) -> Self::Iter<'_> {
        <[T]>::iter(self)
    }
}

// HashSet<T, H>
impl_has_len!(HashSet<T, H>);
impl_has_iter!(HashSet<T, H>, std::collections::hash_set::Iter<'a, T>, &'a T);

// HashMap<K, V, H>
impl_has_len!(HashMap<K, V, H>);
impl_has_iter!(HashMap<K, V, H>, std::collections::hash_map::Iter<'a, K, V>, (&'a K, &'a V));

/// Sample a random element uniformly from a container of known length.
///
/// We do not assume the container is randomly indexable, only that it can be iterated over. The value is cloned.
/// This algorithm is used when the property is indexed, and thus we know the length of the result set.
pub fn sample_single_from_known_length<'a, Container, R, T>(
    rng: &mut R,
    set: &'a Container,
) -> Option<T>
where
    R: Rng,
    Container: HasLen + HasIter<Item<'a> = &'a T>,
    T: Clone + 'static,
{
    let len = set.len();
    if len == 0 {
        return None;
    }
    // This little trick with `u32` makes this function 30% faster.
    let index = rng.random_range(0..len as u32) as usize;
    // The set need not be randomly indexable, so we have to use the `nth` method.
    set.iter().nth(index).cloned()
}

/// Sample a random element uniformly from a container of unknown length.
///
/// We do not assume the container is randomly indexable, only that it can be iterated over. The value is cloned.
///
/// This function implements "Algorithm L" from KIM-HUNG LI
/// Reservoir-Sampling Algorithms of Time Complexity O(n(1 + log(N/n)))
/// <https://dl.acm.org/doi/pdf/10.1145/198429.198435>
///
/// This algorithm is significantly slower than the "known length" algorithm (factor
/// of 10^4). The reservoir algorithm from [`rand`](crate::rand) reduces to the "known length"
/// algorithm when the iterator is an [`ExactSizeIterator`](std::iter::ExactSizeIterator), or more precisely,
/// when `iterator.size_hint()` returns `(k, Some(k))` for some `k`. Otherwise,
/// this algorithm is much faster than the [`rand`](crate::rand) implementation (factor of 100).
// ToDo(RobertJacobsonCDC): This function will take an iterator once the `iter_query_results` API is ready.
pub fn sample_single_l_reservoir<'a, Container, R, T>(rng: &mut R, set: &'a Container) -> Option<T>
where
    R: Rng,
    Container: HasIter<Item<'a> = &'a T>,
    T: Clone + 'static,
{
    let mut chosen_item: Option<T> = None; // the currently selected element
    let mut weight: f64 = rng.random_range(0.0..1.0); // controls skip distance distribution
    let mut position: usize = 0; // current index in data
    let mut next_pick_position: usize = 1; // index of the next item to pick

    set.iter().for_each(|item| {
        position += 1;
        if position == next_pick_position {
            chosen_item = Some(item.clone());
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
pub fn sample_multiple_from_known_length<'a, Container, R, T>(
    rng: &mut R,
    set: &'a Container,
    requested: usize,
) -> Vec<T>
where
    R: Rng,
    Container: HasLen + HasIter<Item<'a> = &'a T>,
    T: Clone + 'static,
{
    let mut indexes = Vec::with_capacity(requested);
    indexes.extend(choose_range(rng, set.len(), requested));
    indexes.sort_unstable();
    let mut index_iterator = indexes.into_iter();
    let mut next_idx = index_iterator.next().unwrap();
    let mut selected = Vec::with_capacity(requested);

    for (idx, item) in set.iter().enumerate() {
        if idx == next_idx {
            selected.push(item.clone());
            if let Some(i) = index_iterator.next() {
                next_idx = i;
            } else {
                break;
            }
        }
    }

    selected
}

/// Sample multiple random elements uniformly without replacement from a container of known length. If
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
// ToDo(RobertJacobsonCDC): This function will take an iterator once the `iter_query_results` API is ready.
pub fn sample_multiple_l_reservoir<'a, Container, R, T>(
    rng: &mut R,
    set: &'a Container,
    requested: usize,
) -> Vec<T>
where
    R: Rng,
    Container: HasIter<Item<'a> = &'a T>,
    T: Clone + 'static,
{
    if requested == 0 {
        return Vec::new();
    }

    let mut weight: f64 = rng.random_range(0.0..1.0); // controls skip distance distribution
    weight = weight.powf(1.0 / requested as f64);
    let mut position: usize = 0; // current index in data
    let mut next_pick_position: usize = 1; // index of the next item to pick
    let mut reservoir = Vec::with_capacity(requested); // the sample reservoir

    set.iter().for_each(|item| {
        position += 1;
        if position == next_pick_position {
            if reservoir.len() == requested {
                let to_remove = rng.random_range(0..reservoir.len());
                reservoir.swap_remove(to_remove);
            }
            reservoir.push(item.clone());

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
    use crate::hashing::{HashSet, HashSetExt};

    // ========== Tests for sample_single_l_reservoir ==========

    #[test]
    fn test_sample_single_l_reservoir_basic() {
        let data: Vec<u32> = (0..1000).collect();
        let seed: u64 = 42;
        let mut rng = StdRng::seed_from_u64(seed);
        let sample = sample_single_l_reservoir(&mut rng, &data);

        // Should return Some value
        assert!(sample.is_some());

        // Value should be in valid range
        let value = sample.unwrap();
        assert!(value < 1000);
    }

    #[test]
    fn test_sample_single_l_reservoir_empty() {
        let data: Vec<u32> = Vec::new();
        let mut rng = StdRng::seed_from_u64(42);
        let sample = sample_single_l_reservoir(&mut rng, &data);

        // Should return None for empty container
        assert!(sample.is_none());
    }

    #[test]
    fn test_sample_single_l_reservoir_single_element() {
        let data: Vec<u32> = vec![42];
        let mut rng = StdRng::seed_from_u64(1);
        let sample = sample_single_l_reservoir(&mut rng, &data);

        // Should return the only element
        assert_eq!(sample, Some(42));
    }

    #[test]
    fn test_sample_single_l_reservoir_uniformity() {
        let population: u32 = 1000;
        let data: Vec<u32> = (0..population).collect();
        let num_runs = 10000;
        let num_bins = 10;
        let mut counts = vec![0usize; num_bins];

        for run in 0..num_runs {
            let mut rng = StdRng::seed_from_u64(42 + run as u64);
            let sample = sample_single_l_reservoir(&mut rng, &data);

            if let Some(value) = sample {
                let bin = (value as usize) / (population as usize / num_bins);
                counts[bin] += 1;
            }
        }

        // Expected count per bin for uniform sampling
        let expected = num_runs as f64 / num_bins as f64;

        // Compute chi-square statistic
        let chi_square: f64 = counts
            .iter()
            .map(|&obs| {
                let diff = (obs as f64) - expected;
                diff * diff / expected
            })
            .sum();

        // Degrees of freedom = num_bins - 1 = 9
        // Critical χ²₀.₉₉₉ for df=9 is 27.877
        let critical = 27.877;

        println!("χ² = {}, counts = {:?}", chi_square, counts);

        assert!(
            chi_square < critical,
            "Single sample fails uniformity test: χ² = {}, counts = {:?}",
            chi_square,
            counts
        );
    }

    #[test]
    fn test_sample_single_l_reservoir_hashset() {
        let mut data = HashSet::new();
        for i in 0..100 {
            data.insert(i);
        }

        let mut rng = StdRng::seed_from_u64(42);
        let sample = sample_single_l_reservoir(&mut rng, &data);

        assert!(sample.is_some());
        let value = sample.unwrap();
        assert!(data.contains(&value));
    }

    // ========== Tests for sample_multiple_l_reservoir ==========

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
        assert!(sample.iter().all(|v| *v < 1000));

        // The sample should not have duplicates
        let unique: HashSet<_> = sample.iter().collect();
        assert_eq!(unique.len(), sample.len());
    }

    #[test]
    fn test_sample_multiple_l_reservoir_empty() {
        let data: Vec<u32> = Vec::new();
        let mut rng = StdRng::seed_from_u64(42);
        let sample = sample_multiple_l_reservoir(&mut rng, &data, 10);

        // Should return empty vector for empty container
        assert_eq!(sample.len(), 0);
    }

    #[test]
    fn test_sample_multiple_l_reservoir_zero_requested() {
        let data: Vec<u32> = (0..100).collect();
        let mut rng = StdRng::seed_from_u64(42);
        let sample = sample_multiple_l_reservoir(&mut rng, &data, 0);

        // Should return empty vector when 0 requested
        assert_eq!(sample.len(), 0);
    }

    #[test]
    fn test_sample_multiple_l_reservoir_requested_exceeds_population() {
        let data: Vec<u32> = (0..50).collect();
        let requested = 100;
        let mut rng = StdRng::seed_from_u64(42);
        let sample = sample_multiple_l_reservoir(&mut rng, &data, requested);

        // Should return all available items when requested > population
        assert_eq!(sample.len(), 50);

        // All elements should be unique
        let unique: HashSet<_> = sample.iter().collect();
        assert_eq!(unique.len(), 50);

        // All elements should be from the original data
        assert!(sample.iter().all(|v| *v < 50));
    }

    #[test]
    fn test_sample_multiple_l_reservoir_exact_population() {
        let data: Vec<u32> = (0..100).collect();
        let mut rng = StdRng::seed_from_u64(42);
        let sample = sample_multiple_l_reservoir(&mut rng, &data, 100);

        // Should return all elements when requested == population
        assert_eq!(sample.len(), 100);

        let unique: HashSet<_> = sample.iter().collect();
        assert_eq!(unique.len(), 100);
    }

    #[test]
    fn test_sample_multiple_l_reservoir_single_element() {
        let data: Vec<u32> = vec![42];
        let mut rng = StdRng::seed_from_u64(1);
        let sample = sample_multiple_l_reservoir(&mut rng, &data, 1);

        assert_eq!(sample.len(), 1);
        assert_eq!(sample[0], 42);
    }

    #[test]
    fn test_sample_multiple_l_reservoir_hashset() {
        let mut data = HashSet::new();
        for i in 0..100 {
            data.insert(i);
        }

        let mut rng = StdRng::seed_from_u64(42);
        let sample = sample_multiple_l_reservoir(&mut rng, &data, 10);

        assert_eq!(sample.len(), 10);

        // All sampled values should be in the original set
        assert!(sample.iter().all(|v| data.contains(v)));

        // No duplicates
        let unique: HashSet<_> = sample.iter().collect();
        assert_eq!(unique.len(), 10);
    }

    #[test]
    fn test_sample_multiple_l_reservoir_small_sample() {
        let data: Vec<u32> = (0..1000).collect();
        let requested = 5;
        let mut rng = StdRng::seed_from_u64(42);
        let sample = sample_multiple_l_reservoir(&mut rng, &data, requested);

        assert_eq!(sample.len(), requested);

        // No duplicates
        let unique: HashSet<_> = sample.iter().collect();
        assert_eq!(unique.len(), requested);
    }

    #[test]
    fn test_sample_multiple_l_reservoir_large_sample() {
        let data: Vec<u32> = (0..1000).collect();
        let requested = 900;
        let mut rng = StdRng::seed_from_u64(42);
        let sample = sample_multiple_l_reservoir(&mut rng, &data, requested);

        assert_eq!(sample.len(), requested);

        // No duplicates
        let unique: HashSet<_> = sample.iter().collect();
        assert_eq!(unique.len(), requested);
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
            let sample = sample_multiple_l_reservoir(&mut rng, &data, requested);

            // Partition range 0..population into 10 equal-width bins
            let mut counts = [0usize; 10];
            for &value in &sample {
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

    // Test that each element has equal probability of being selected
    #[test]
    fn test_sample_multiple_l_reservoir_element_probability() {
        let population: u32 = 100;
        let data: Vec<u32> = (0..population).collect();
        let requested = 10;
        let num_runs = 10000;
        let mut selection_counts = vec![0usize; population as usize];

        for run in 0..num_runs {
            let mut rng = StdRng::seed_from_u64(42 + run as u64);
            let sample = sample_multiple_l_reservoir(&mut rng, &data, requested);

            for &value in &sample {
                selection_counts[value as usize] += 1;
            }
        }

        // Each element should be selected with probability requested/population
        // Expected count per element
        let expected = (num_runs * requested) as f64 / population as f64;

        // Compute chi-square statistic
        let chi_square: f64 = selection_counts
            .iter()
            .map(|&obs| {
                let diff = (obs as f64) - expected;
                diff * diff / expected
            })
            .sum();

        // Degrees of freedom = population - 1 = 99
        // Critical χ²₀.₉₉₉ for df=99 is approximately 140.169
        let critical = 140.169;

        println!(
            "χ² = {}, expected = {}, min = {}, max = {}",
            chi_square,
            expected,
            selection_counts.iter().min().unwrap(),
            selection_counts.iter().max().unwrap()
        );

        assert!(
            chi_square < critical,
            "Element selection probabilities are not uniform: χ² = {}",
            chi_square
        );
    }

    // Test reproducibility with same seed
    #[test]
    fn test_sample_multiple_l_reservoir_reproducibility() {
        let data: Vec<u32> = (0..1000).collect();
        let test_sizes = [1, 2, 5, 10, 100, 500];

        for &requested in &test_sizes {
            let seed: u64 = 12345;

            let mut rng1 = StdRng::seed_from_u64(seed);
            let sample1 = sample_multiple_l_reservoir(&mut rng1, &data, requested);

            let mut rng2 = StdRng::seed_from_u64(seed);
            let sample2 = sample_multiple_l_reservoir(&mut rng2, &data, requested);

            // Verify correct sample size
            assert_eq!(
                sample1.len(),
                requested,
                "Sample size {} doesn't match requested size {}",
                sample1.len(),
                requested
            );
            assert_eq!(
                sample2.len(),
                requested,
                "Sample size {} doesn't match requested size {}",
                sample2.len(),
                requested
            );

            // Same seed should produce identical samples
            assert_eq!(
                sample1, sample2,
                "Reproducibility failed for requested={}",
                requested
            );
        }
    }

    #[test]
    fn test_sample_single_l_reservoir_reproducibility() {
        let data: Vec<u32> = (0..1000).collect();
        let seed: u64 = 12345;

        let mut rng1 = StdRng::seed_from_u64(seed);
        let sample1 = sample_single_l_reservoir(&mut rng1, &data);

        let mut rng2 = StdRng::seed_from_u64(seed);
        let sample2 = sample_single_l_reservoir(&mut rng2, &data);

        // Same seed should produce identical samples
        assert_eq!(sample1, sample2);
    }
}

use criterion::{criterion_group, criterion_main, Criterion};
use ixa::rand::{prelude::ThreadRng, seq::IteratorRandom, thread_rng, Rng};
use ixa::random::{
    sample_multiple_from_known_length, sample_multiple_l_reservoir,
    sample_single_from_known_length, sample_single_l_reservoir,
};
use std::hint::black_box;

/// A wrapper around any iterator that prevents it from exposing `ExactSizeIterator`, `DoubleEndedIterator`, etc.
/// This is needed to test the "slow path" with `rand`'s implementation, as `rand` has an optimization for when
/// `iterator.size_hint()` returns `(k, Some(k))`.
pub struct NonExactSize<I> {
    inner: I,
}

impl<I> NonExactSize<I> {
    pub fn new(inner: I) -> Self {
        Self { inner }
    }
}

impl<I> Iterator for NonExactSize<I>
where
    I: Iterator,
{
    type Item = I::Item;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
}

fn setup() -> (Vec<u8>, Vec<usize>, ThreadRng) {
    let mut rng: ThreadRng = thread_rng();
    // The number of items to choose out of the data set for multiple sampling
    let mut counts: Vec<usize> = Vec::with_capacity(1000);
    for _ in 0..1000 {
        counts.push(rng.gen_range(5..100));
    }
    // A data set of numbers to simulate sampling from a population (memory access patterns)
    let mut data: Vec<u8> = Vec::with_capacity(100_000);
    for _ in 0..100_000 {
        data.push(rng.gen_range(0..100));
    }

    (data, counts, rng)
}

pub fn criterion_benchmark(criterion: &mut Criterion) {
    let mut criterion = criterion.benchmark_group("algorithm_benches");
    let (data, counts, mut rng) = setup();

    // This algorithm is used when the property is indexed, and thus we know the length of the result set.
    criterion.bench_function("algorithm_sampling_single_known_length", |bencher| {
        bencher.iter(|| {
            // Treat inputs as opaque at the start of the iteration
            let rng = black_box(&mut rng);
            let mut data_iter = black_box(data.iter().copied());
            black_box(sample_single_from_known_length(
                rng,
                &mut data_iter,
                100_000,
            ));
        });
    });

    // This algorithm is significantly slower than the "known length" algorithm (factor
    // of 10^4). The reservoir algorithm from `rand` reduces to the "known length`
    // algorithm when the iterator is an `ExactSizeIterator`, or more precisely,
    // when `iterator.size_hint()` returns `(k, Some(k))` for some `k`. Otherwise,
    // this algorithm is much faster than the `rand` implementation (factor of 100).
    criterion.bench_function("algorithm_sampling_single_l_reservoir", |bencher| {
        bencher.iter(|| {
            // Treat inputs as opaque at the start of the iteration
            let rng = black_box(&mut rng);
            let mut data_iter = black_box(data.iter().copied());

            black_box(sample_single_l_reservoir(rng, &mut data_iter));
        });
    });

    // The implementation of this algorithm actually reduces to the "known length" algorithm above in the
    // case that the iterator is an `ExactSizeIterator`, or more precisely, when `iterator.size_hint()`
    // returns `(k, Some(k))` for some `k`.
    criterion.bench_function("algorithm_sampling_single_rand_reservoir", |bencher| {
        bencher.iter(|| {
            // Treat inputs as opaque at the start of the iteration
            let rng = black_box(&mut rng);
            let data_iter = black_box(data.iter().copied());
            let iterator = NonExactSize::new(data_iter);

            // Use the `rand` crate's reservoir sampling implementation
            let selected = iterator.choose(rng);
            black_box(selected);
        });
    });

    // This "algorithm" is used when the property is indexed, and thus we know the length of the result set.
    // For very small `requested` values (<=5), this algorithm is faster than reservoir because it doesn't
    // iterate over the entire set.
    criterion.bench_function("algorithm_sampling_multiple_known_length", |bencher| {
        let mut count_idx: usize = 0;

        bencher.iter(|| {
            // Treat inputs as opaque at the start of the iteration
            let rng = black_box(&mut rng);
            let mut data_iter = black_box(data.iter().copied());
            let requested = counts[count_idx];

            let selected =
                sample_multiple_from_known_length(rng, &mut data_iter, 100_000, requested);

            assert_eq!(selected.len(), requested);
            count_idx = (count_idx + 1) % 1000;
            black_box(selected);
        });
    });

    // This algorithm is significantly faster than the reservoir algorithm in `rand` and is
    // on par with the "known length" algorithm for large `requested` values.
    criterion.bench_function("algorithm_sampling_multiple_l_reservoir", |bencher| {
        let mut count_idx: usize = 0;

        bencher.iter(|| {
            // Treat inputs as opaque at the start of the iteration
            let rng = black_box(&mut rng);
            let mut data_iter = black_box(data.iter().copied());
            let requested = counts[count_idx];

            let reservoir = sample_multiple_l_reservoir(rng, &mut data_iter, counts[count_idx]);
            assert_eq!(reservoir.len(), requested);
            count_idx = (count_idx + 1) % 1000;
            black_box(reservoir);
        });
    });

    #[cfg(feature = "alternative_algorithm_benches")]
    criterion.bench_function("algorithm_sampling_multiple_rand_reservoir", |bencher| {
        let mut count_idx: usize = 0;
        bencher.iter(|| {
            // Treat inputs as opaque at the start of the iteration
            let rng = black_box(&mut rng);
            let data_iter = black_box(data.iter().copied());
            let requested = counts[count_idx];
            // It turns out the following line makes no difference in performance.
            // let iterator = NonExactSize::new(data_iter);

            // Use the `rand` crate's reservoir sampling implementation
            let selected = data_iter.choose_multiple(rng, requested);
            assert_eq!(selected.len(), requested);
            count_idx = (count_idx + 1) % 1000;
            black_box(selected);
        });
    });

    criterion.finish()
}

criterion_group!(algorithm_benches, criterion_benchmark);
criterion_main!(algorithm_benches);

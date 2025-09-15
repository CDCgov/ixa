use criterion::{criterion_group, criterion_main, Criterion};
use ixa::rand::{seq::index::sample as choose_range, seq::IteratorRandom, thread_rng, Rng};
use std::hint::black_box;

pub fn criterion_benchmark(criterion: &mut Criterion) {
    let mut rng = thread_rng();
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

    // This "algorithm" is used when the property is indexed, and thus we know the length of the result set.
    criterion.bench_function("algorithm_sampling_single_known_length", |bencher| {
        bencher.iter(|| {
            // Treat inputs as opaque at the start of the iteration
            let rng = black_box(&mut rng);
            let data = black_box(&data);

            // This little trick with `u32` makes this function 30% faster.
            let index = rng.gen_range(0..data.len() as u32) as usize;
            // The set is not randomly indexable, so we have to use the `nth` method.
            let selected = data.iter().nth(index).unwrap();

            black_box(selected);
        });
    });

    // This algorithm is significantly slower than either the "known length" algorithm or the
    // reservoir algorithm from `rand` when the iterator is an `ExactSizeIterator`.
    criterion.bench_function("algorithm_sampling_single_l_reservoir", |bencher| {
        bencher.iter(|| {
            // Treat inputs as opaque at the start of the iteration
            let rng = black_box(&mut rng);
            let data = black_box(&data);

            let mut chosen_item: Option<u8> = None; // the currently selected element
            let mut weight: f64 = rng.gen_range(0.0..1.0); // controls skip distance distribution
            let mut position: usize = 0; // current index in data
            let mut next_pick_position: usize = 1; // index of the next item to pick

            data.iter().for_each(|&item| {
                position += 1;
                if position == next_pick_position {
                    chosen_item = Some(item);
                    next_pick_position += (f64::ln(rng.gen_range(0.0..1.0)) / f64::ln(1.0 - weight))
                        .floor() as usize
                        + 1;
                    weight *= rng.gen_range(0.0..1.0);
                }
            });

            black_box(chosen_item);
        });
    });

    // The implementation of this algorithm actually reduces to the "known length" algorithm above in the
    // case that the iterator is an `ExactSizeIterator`.
    criterion.bench_function("algorithm_sampling_single_rand_reservoir", |bencher| {
        bencher.iter(|| {
            // Treat inputs as opaque at the start of the iteration
            let rng = black_box(&mut rng);
            let data = black_box(&data);

            // Use the `rand` crate's reservoir sampling implementation
            let selected = data.iter().choose(rng);
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
            let data = black_box(&data);

            let requested = counts[count_idx];

            let mut indexes = Vec::with_capacity(requested);
            indexes.extend(choose_range(rng, data.len(), requested).into_iter());
            indexes.sort_unstable();
            let mut index_iterator = indexes.into_iter();
            let mut next_idx = index_iterator.next().unwrap();
            let mut selected = Vec::with_capacity(requested);

            for (idx, person_id) in data.iter().enumerate() {
                if idx == next_idx {
                    selected.push(*person_id);
                    if let Some(i) = index_iterator.next() {
                        next_idx = i;
                    } else {
                        break;
                    }
                }
            }

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
            let data = black_box(&data);

            let mut weight: f64 = rng.gen_range(0.0..1.0); // controls skip distance distribution
            let mut position: usize = 0; // current index in data
            let mut next_pick_position: usize = 1; // index of the next item to pick
            let requested = counts[count_idx]; // target reservoir size
            let mut reservoir: Vec<u8> = Vec::with_capacity(requested); // the sample reservoir

            data.iter().for_each(|&item| {
                position += 1;
                if position == next_pick_position {
                    if reservoir.len() == requested {
                        let to_remove = rng.gen_range(0..reservoir.len());
                        reservoir.swap_remove(to_remove);
                    }
                    reservoir.push(item);

                    if reservoir.len() == requested {
                        next_pick_position += (f64::ln(rng.gen_range(0.0..1.0))
                            / f64::ln(1.0 - weight))
                        .floor() as usize
                            + 1;
                        weight *= rng.gen_range(0.0..1.0);
                    } else {
                        next_pick_position += 1;
                    }
                }
            });
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
            let data = black_box(&data);
            let requested = counts[count_idx];

            // Use the `rand` crate's reservoir sampling implementation
            let selected = data.iter().copied().choose_multiple(rng, requested);
            assert_eq!(selected.len(), requested);
            count_idx = (count_idx + 1) % 1000;
            black_box(selected);
        });
    });
}

criterion_group!(algorithm_benches, criterion_benchmark);
criterion_main!(algorithm_benches);

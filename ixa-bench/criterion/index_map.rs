//! Compares `IndexableMap` against alternative storage choices: `ixa::HashMap`
//! (the natural sparse-key alternative) and `Vec<Option<V>>` (the map's own
//! backing store — isolates the cost of the wrapping API).

use std::hint::black_box;

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use ixa::indexable_map::IndexableMap;
use ixa::HashMap;
use rand::rngs::SmallRng;
use rand::{Rng, SeedableRng};

const HIGH_N: usize = 1_000_000;
const LOOKUPS: usize = 100_000;

/// Pseudo-random keys in `0..n`. Avoids sequential access so the hardware
/// prefetcher can't mask the cost of the actual lookup.
fn lookup_indices(n: usize) -> Vec<usize> {
    let mut rng = SmallRng::seed_from_u64(42);
    (0..LOOKUPS).map(|_| rng.random_range(0..n)).collect()
}

fn full_index_map(n: usize) -> IndexableMap<usize, usize> {
    let mut map = IndexableMap::with_capacity(n);
    for index in 0..n {
        map.insert(index, index);
    }
    map
}

/// Every 10th index occupied. The map still allocates `n` slots; only `n/10`
/// carry values. Used to measure scan-and-skip cost on a sparse map.
fn tenth_occupied_index_map(n: usize) -> IndexableMap<usize, usize> {
    let mut map = IndexableMap::with_capacity(n);
    for index in (0..n).step_by(10) {
        map.insert(index, index);
    }
    map
}

fn full_hash_map(n: usize) -> HashMap<usize, usize> {
    let mut map: HashMap<usize, usize> = HashMap::with_capacity_and_hasher(n, Default::default());
    for index in 0..n {
        map.insert(index, index);
    }
    map
}

fn criterion_benchmark(criterion: &mut Criterion) {
    // INSERT: how expensive is filling the map from empty? All four variants
    // pre-allocate capacity for `n` items, so growth/rehashing isn't measured.
    let mut insert_group = criterion.benchmark_group("index_map_insert_high_n");
    insert_group.sample_size(10);
    insert_group.throughput(Throughput::Elements(HIGH_N as u64));

    insert_group.bench_with_input(
        BenchmarkId::new("index_map_sequential", HIGH_N),
        &HIGH_N,
        |bencher, &n| {
            bencher.iter(|| {
                let mut map = IndexableMap::with_capacity(n);
                for index in 0..n {
                    map.insert(black_box(index), black_box(index));
                }
                black_box(map);
            });
        },
    );

    insert_group.bench_with_input(
        BenchmarkId::new("hash_map_sequential", HIGH_N),
        &HIGH_N,
        |bencher, &n| {
            bencher.iter(|| {
                let mut map: HashMap<usize, usize> =
                    HashMap::with_capacity_and_hasher(n, Default::default());
                for index in 0..n {
                    map.insert(black_box(index), black_box(index));
                }
                black_box(map);
            });
        },
    );

    insert_group.bench_with_input(
        BenchmarkId::new("vec_sequential", HIGH_N),
        &HIGH_N,
        |bencher, &n| {
            bencher.iter(|| {
                let mut values = Vec::with_capacity(n);
                for index in 0..n {
                    values.push(black_box(index));
                }
                black_box(values);
            });
        },
    );

    insert_group.bench_with_input(
        BenchmarkId::new("vec_option_sequential", HIGH_N),
        &HIGH_N,
        |bencher, &n| {
            bencher.iter(|| {
                let mut values = Vec::with_capacity(n);
                for index in 0..n {
                    values.push(Some(black_box(index)));
                }
                black_box(values);
            });
        },
    );
    insert_group.finish();

    let lookup_keys = lookup_indices(HIGH_N);
    let index_map = full_index_map(HIGH_N);
    let hash_map = full_hash_map(HIGH_N);
    let option_values = (0..HIGH_N).map(Some).collect::<Vec<_>>();

    // LOOKUP: random `get` on a fully-populated map. The case where
    // IndexableMap is meant to win against HashMap (array index, no hashing).
    // `vec_option_random_get` is the no-API-overhead floor.
    let mut lookup_group = criterion.benchmark_group("index_map_lookup_high_n");
    lookup_group.throughput(Throughput::Elements(lookup_keys.len() as u64));

    lookup_group.bench_function("index_map_random_get", |bencher| {
        bencher.iter(|| {
            let mut sum = 0usize;
            for index in &lookup_keys {
                sum = sum.wrapping_add(*index_map.get(black_box(*index)).unwrap());
            }
            black_box(sum);
        });
    });

    lookup_group.bench_function("hash_map_random_get", |bencher| {
        bencher.iter(|| {
            let mut sum = 0usize;
            for index in &lookup_keys {
                sum = sum.wrapping_add(*hash_map.get(black_box(index)).unwrap());
            }
            black_box(sum);
        });
    });

    lookup_group.bench_function("vec_option_random_get", |bencher| {
        bencher.iter(|| {
            let mut sum = 0usize;
            for index in &lookup_keys {
                sum = sum.wrapping_add(option_values[black_box(*index)].unwrap());
            }
            black_box(sum);
        });
    });
    lookup_group.finish();

    let tenth_index_map = tenth_occupied_index_map(HIGH_N);

    // ITER: walk the storage end-to-end. `vec_option_iter_full` isolates
    // API overhead; `tenth_occupied` shows scan-and-skip cost on a sparse map.
    let mut iter_group = criterion.benchmark_group("index_map_iter_high_n");
    iter_group.throughput(Throughput::Elements(HIGH_N as u64));

    iter_group.bench_function("index_map_iter_full", |bencher| {
        bencher.iter(|| {
            let sum = index_map
                .values()
                .fold(0usize, |acc, value| acc.wrapping_add(*value));
            black_box(sum);
        });
    });

    iter_group.bench_function("index_map_iter_tenth_occupied", |bencher| {
        bencher.iter(|| {
            let sum = tenth_index_map
                .values()
                .fold(0usize, |acc, value| acc.wrapping_add(*value));
            black_box(sum);
        });
    });

    iter_group.bench_function("hash_map_iter_full", |bencher| {
        bencher.iter(|| {
            let sum = hash_map
                .values()
                .fold(0usize, |acc, value| acc.wrapping_add(*value));
            black_box(sum);
        });
    });

    iter_group.bench_function("vec_option_iter_full", |bencher| {
        bencher.iter(|| {
            let sum = option_values
                .iter()
                .fold(0usize, |acc, value| acc.wrapping_add(value.unwrap()));
            black_box(sum);
        });
    });

    iter_group.finish();
}

criterion_group!(index_map_benches, criterion_benchmark);
criterion_main!(index_map_benches);

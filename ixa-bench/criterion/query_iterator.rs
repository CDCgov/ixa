use std::hint::black_box;

use criterion::{criterion_group, criterion_main, Criterion};
use ixa::prelude::*;

// mise bench:criterion query_iterator
const BENCH_NAME: &str = "query_iterator";
const SEED: u64 = 42;
const N_PEOPLE: usize = 100_000;
define_person_property!(AgeGroup, u8);
define_rng!(AssignRng);

fn setup(n: usize, age_groups: Vec<(u8, f64)>) -> Result<Context, IxaError> {
    let mut context = Context::new();
    context.init_random(SEED);
    context.index_property(AgeGroup);

    let (ages, weights): (Vec<u8>, Vec<f64>) = age_groups.into_iter().unzip();
    for _ in 0..n {
        let index = context.sample_weighted(AssignRng, &weights);
        context.add_person((AgeGroup, ages[index]))?;
    }

    Ok(context)
}

pub fn criterion_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group(BENCH_NAME);

    let context = setup(
        N_PEOPLE,
        vec![(0, 18.2), (15, 13.0), (25, 39.9), (55, 12.9), (65, 16.8)],
    )
    .unwrap();

    // Iterator over all people, completing the iteration.
    group.bench_function("unfiltered_complete", |bencher| {
        bencher.iter(|| {
            for _ in context.iter_query(()) {
                black_box(());
            }
        });
    });

    // Iterate and break
    group.bench_function("unfiltered_with_break", |bencher| {
        bencher.iter(|| {
            let mut count = 0;
            for _ in context.iter_query(()) {
                if black_box(count == N_PEOPLE / 10) {
                    break;
                }
                count += 1;
            }
            black_box(count);
        });
    });

    // Iterate over filtered
    group.bench_function("filtered", |bencher| {
        bencher.iter(|| {
            for _ in black_box(context.iter_query((AgeGroup, 25))) {
                black_box(());
            }
        });
    });

    // Iterate and break filtered
    group.bench_function("filtered_with_break", |bencher| {
        bencher.iter(|| {
            let mut count = 5;
            for _ in black_box(context.iter_query((AgeGroup, 25))) {
                if black_box(count == 5) {
                    break;
                }
                count += 1;
            }
            black_box(count);
        });
    });

    group.finish();
}

criterion_group!(query_iterator_benches, criterion_benchmark);
criterion_main!(query_iterator_benches);

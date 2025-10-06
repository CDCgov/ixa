use criterion::{criterion_group, criterion_main, Criterion};
use ixa::prelude::*;
use ixa::rand::{rngs::StdRng, Rng, SeedableRng};
use ixa::PersonId;
use std::hint::black_box;

define_rng!(SampleBenchRng);

const SEED: u64 = 42;

define_person_property!(Property10, u8, |context: &Context, _person: PersonId| {
    context.sample_range(SampleBenchRng, 0..10)
});
define_person_property!(Property100, u8, |context: &Context, _person: PersonId| {
    context.sample_range(SampleBenchRng, 0..100)
});

fn setup() -> (Context, Vec<u8>) {
    let mut rng = StdRng::seed_from_u64(SEED);
    let mut context = Context::new();

    // Seed context RNGs for deterministic property generation
    context.init_random(SEED);

    context.index_property(Property10);
    context.index_property(Property100);

    // The number of items to choose out of the data set for multiple sampling
    let mut counts: Vec<u8> = Vec::with_capacity(1000);
    for _ in 0..1000 {
        counts.push(rng.random_range(5..100));
    }

    // Add population
    for _ in 0..100_000 {
        let _ = context.add_person(());
    }

    (context, counts)
}

pub fn criterion_benchmark(criterion: &mut Criterion) {
    let mut criterion = criterion.benchmark_group("sample_people");
    let (context, counts) = setup();

    // Sampling one person when the property is indexed, and thus we know the length of the result set.
    criterion.bench_function("sampling_single_known_length", |bencher| {
        bencher.iter(|| {
            let counts = black_box(&counts);

            for value in counts {
                let _selected = black_box(
                    context.sample_person(SampleBenchRng, black_box((Property100, *value))),
                );
            }
        });
    });

    // Sampling one person when the query is not a single indexed property/multi-property. The result
    // set is not realized, so this is the reservoir sampling case.
    criterion.bench_function("sampling_single_l_reservoir", |bencher| {
        bencher.iter(|| {
            let counts = black_box(&counts);

            for value in counts {
                let _selected = black_box(context.sample_person(
                    SampleBenchRng,
                    black_box(((Property10, *value % 10), (Property100, *value))),
                ));
            }
        });
    });

    // Sampling several people when the property is indexed, and thus we know the length of the result set.
    criterion.bench_function("sampling_multiple_known_length", |bencher| {
        bencher.iter(|| {
            let counts = black_box(&counts);

            for value in counts {
                let _selected = black_box(context.sample_people(
                    SampleBenchRng,
                    black_box((Property100, *value)),
                    *value as usize,
                ));
            }
        });
    });

    // Sampling several people when the query is not a single indexed property/multi-property. The result
    // set is not realized, so this is the reservoir sampling case.
    criterion.bench_function("sampling_multiple_l_reservoir", |bencher| {
        bencher.iter(|| {
            let counts = black_box(&counts);

            for value in counts {
                let _selected = black_box(context.sample_people(
                    SampleBenchRng,
                    black_box(((Property10, *value % 10), (Property100, *value))),
                    *value as usize,
                ));
            }
        });
    });

    criterion.finish()
}

criterion_group!(sampling_benches, criterion_benchmark);
criterion_main!(sampling_benches);

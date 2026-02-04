use std::hint::black_box;

use criterion::{criterion_group, criterion_main, Criterion};
use ixa::prelude::*;
use ixa::rand::rngs::StdRng;
use ixa::rand::{Rng, SeedableRng};
use ixa::PersonId;

define_rng!(SampleBenchRng);

const SEED: u64 = 42;

define_person_property!(Property10, u8, |context: &Context, _person: PersonId| {
    context.sample_range(SampleBenchRng, 0..10)
});
define_person_property!(Property100, u8, |context: &Context, _person: PersonId| {
    context.sample_range(SampleBenchRng, 0..100)
});

// Entity Properties
define_entity!(Animal);
define_property!(struct AProperty10(u8), Animal);
define_property!(struct AProperty100(u8), Animal);

fn setup() -> (Context, Vec<u8>) {
    let mut rng = StdRng::seed_from_u64(SEED);
    let mut context = Context::new();

    // Seed context RNGs for deterministic property generation
    context.init_random(SEED);

    context.index_person_property(Property10);
    context.index_person_property(Property100);

    context.index_property::<Animal, AProperty10>();
    context.index_property::<Animal, AProperty100>();

    // The number of items to choose out of the data set for multiple sampling
    let mut counts: Vec<u8> = Vec::with_capacity(1000);
    for _ in 0..1000 {
        counts.push(rng.random_range(5..100));
    }

    // Add population
    for _ in 0..100_000 {
        let _ = context.add_person(());
        context
            .add_entity(all!(
                Animal,
                AProperty10(context.sample_range(SampleBenchRng, 0..10)),
                AProperty100(context.sample_range(SampleBenchRng, 0..100))
            ))
            .unwrap();
    }
    // The Entity Properties have to be initialized from the start.
    // This query forces the initialization of the Person Properties
    // so we are comparing apples to apples in the benchmarks.
    context.with_query_people_results((Property10, 0), &mut |people_set| {
        black_box(people_set.len());
    });
    context.with_query_people_results((Property100, 0), &mut |people_set| {
        black_box(people_set.len());
    });

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
    criterion.bench_function("sampling_single_known_length_entities", |bencher| {
        bencher.iter(|| {
            let counts = black_box(&counts);

            for value in counts {
                let _selected = black_box(context.sample_entity(
                    SampleBenchRng,
                    black_box(all!(Animal, AProperty100(*value))),
                ));
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
    criterion.bench_function("sampling_single_l_reservoir_entities", |bencher| {
        bencher.iter(|| {
            let counts = black_box(&counts);

            for value in counts {
                let _selected = black_box(context.sample_entity(
                    SampleBenchRng,
                    black_box(all!(Animal, AProperty10(*value % 10), AProperty100(*value))),
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
                    *black_box(value) as usize,
                ));
            }
        });
    });
    criterion.bench_function("sampling_multiple_known_length_entities", |bencher| {
        bencher.iter(|| {
            let counts = black_box(&counts);

            for value in counts {
                let _selected = black_box(context.sample_entities(
                    SampleBenchRng,
                    black_box(all!(Animal, AProperty100(*value))),
                    *black_box(value) as usize,
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
                    *black_box(value) as usize,
                ));
            }
        });
    });
    criterion.bench_function("sampling_multiple_l_reservoir_entities", |bencher| {
        bencher.iter(|| {
            let counts = black_box(&counts);

            for value in counts {
                let _selected = black_box(context.sample_entities(
                    SampleBenchRng,
                    black_box(all!(Animal, AProperty10(*value % 10), AProperty100(*value))),
                    *black_box(value) as usize,
                ));
            }
        });
    });

    criterion.finish()
}

criterion_group!(sampling_benches, criterion_benchmark);
criterion_main!(sampling_benches);

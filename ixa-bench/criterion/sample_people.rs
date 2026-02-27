use std::hint::black_box;

use criterion::{criterion_group, criterion_main, Criterion};
use ixa::prelude::*;
use ixa::rand::rngs::StdRng;
use ixa::rand::{Rng, SeedableRng};

define_rng!(SampleBenchRng);

const SEED: u64 = 42;

// Entity Properties
define_entity!(Person);
define_property!(struct Property10(u8), Person);
define_property!(struct Property100(u8), Person);
define_property!(struct Unindexed10(u8), Person);
define_property!(struct Age(u8), Person);
define_derived_property!(
    struct AgeGroupFoi(u8),
    Person,
    [Age],
    [],
    |age| {
        if age.0 <= 1 {
            AgeGroupFoi(0)
        } else if age.0 <= 65 {
            AgeGroupFoi(1)
        } else {
            AgeGroupFoi(2)
        }
    }
);

fn setup() -> (Context, Vec<u8>) {
    let mut rng = StdRng::seed_from_u64(SEED);
    let mut context = Context::new();

    // Seed context RNGs for deterministic property generation
    context.init_random(SEED);

    context.index_property::<Person, Property10>();
    context.index_property::<Person, Property100>();

    // The number of items to choose out of the data set for multiple sampling
    let mut counts: Vec<u8> = Vec::with_capacity(1000);
    for _ in 0..1000 {
        counts.push(rng.random_range(5..100));
    }

    // Add population
    for _ in 0..100_000 {
        context
            .add_entity((
                Property10(context.sample_range(SampleBenchRng, 0..10)),
                Property100(context.sample_range(SampleBenchRng, 0..100)),
                Unindexed10(context.sample_range(SampleBenchRng, 0..10)),
                Age(context.sample_range(SampleBenchRng, 0..100)),
            ))
            .unwrap();
    }

    (context, counts)
}

pub fn criterion_benchmark(criterion: &mut Criterion) {
    let mut criterion = criterion.benchmark_group("sampling");
    let (context, counts) = setup();

    // Sampling one entity when the property is indexed, and thus we know the length of the result set.
    criterion.bench_function("sampling_single_known_length_entities", |bencher| {
        bencher.iter(|| {
            let counts = black_box(&counts);

            for value in counts {
                let _selected = black_box(
                    context.sample_entity(SampleBenchRng, black_box((Property100(*value),))),
                );
            }
        });
    });

    criterion.bench_function(
        "count_and_sampling_single_known_length_entities",
        |bencher| {
            bencher.iter(|| {
                let counts = black_box(&counts);

                for value in counts {
                    let _selected = black_box(context.count_and_sample_entity(
                        SampleBenchRng,
                        black_box((Property100(*value),)),
                    ));
                }
            });
        },
    );

    // Sampling one entity when the query is not a single indexed property/multi-property. The result
    // set is not realized, so this is the reservoir sampling case.
    criterion.bench_function("sampling_single_l_reservoir_entities", |bencher| {
        bencher.iter(|| {
            let counts = black_box(&counts);

            for value in counts {
                let _selected = black_box(context.sample_entity(
                    SampleBenchRng,
                    black_box((Property10(*value % 10), Property100(*value))),
                ));
            }
        });
    });

    // Sampling several entities when the property is indexed, and thus we know the length of the result set.
    criterion.bench_function("sampling_multiple_known_length_entities", |bencher| {
        bencher.iter(|| {
            let counts = black_box(&counts);

            for value in counts {
                let _selected = black_box(context.sample_entities(
                    SampleBenchRng,
                    black_box((Property100(*value),)),
                    *black_box(value) as usize,
                ));
            }
        });
    });

    // Sampling several entities when the query is not a single indexed property/multi-property. The result
    // set is not realized, so this is the reservoir sampling case.
    criterion.bench_function("sampling_multiple_l_reservoir_entities", |bencher| {
        bencher.iter(|| {
            let counts = black_box(&counts);

            for value in counts {
                let _selected = black_box(context.sample_entities(
                    SampleBenchRng,
                    black_box((Property10(*value % 10), Property100(*value))),
                    *black_box(value) as usize,
                ));
            }
        });
    });

    // Sampling one entity when the query is on an unindexed property. The source iterator is a
    // PropertyVecIter, which must scan the property's value vector.
    criterion.bench_function("sampling_single_unindexed_entities", |bencher| {
        bencher.iter(|| {
            let counts = black_box(&counts);

            for value in counts {
                let _selected = black_box(
                    context.sample_entity(SampleBenchRng, black_box((Unindexed10(*value % 10),))),
                );
            }
        });
    });

    // Sampling several entities when the query is on an unindexed property. The source iterator is a
    // PropertyVecIter, which must scan the property's value vector.
    criterion.bench_function("sampling_multiple_unindexed_entities", |bencher| {
        bencher.iter(|| {
            let counts = black_box(&counts);

            for value in counts {
                let _selected = black_box(context.sample_entities(
                    SampleBenchRng,
                    black_box((Unindexed10(*value % 10),)),
                    *black_box(value) as usize,
                ));
            }
        });
    });

    // Sampling one entity for an unindexed concrete + unindexed derived query.
    criterion.bench_function(
        "sampling_single_unindexed_concrete_plus_derived_entities",
        |bencher| {
            bencher.iter(|| {
                let counts = black_box(&counts);

                for value in counts {
                    let _selected = black_box(context.sample_entity(
                        SampleBenchRng,
                        black_box((Unindexed10(*value % 10), AgeGroupFoi(*value % 3))),
                    ));
                }
            });
        },
    );

    criterion.bench_function(
        "count_and_sampling_single_unindexed_concrete_plus_derived_entities",
        |bencher| {
            bencher.iter(|| {
                let counts = black_box(&counts);

                for value in counts {
                    let _selected = black_box(context.count_and_sample_entity(
                        SampleBenchRng,
                        black_box((Unindexed10(*value % 10), AgeGroupFoi(*value % 3))),
                    ));
                }
            });
        },
    );

    criterion.finish()
}

criterion_group!(sampling_benches, criterion_benchmark);
criterion_main!(sampling_benches);

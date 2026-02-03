use std::hint::black_box;

use criterion::{criterion_group, criterion_main, BatchSize, Criterion};
use ixa::context::Context;
use ixa::prelude::*;
use ixa_bench::generate_population::generate_population_with_seed;

const SEED: u64 = 42;

// Entity and Properties
define_entity!(Person);
define_property!(struct Age(u8), Person);
define_property!(struct HomeId(u32), Person);
define_property!(struct SchoolId(u32), Person);
define_property!(struct WorkplaceId(u32), Person);
define_derived_property!(
    enum AgeGroupRisk {
        Newborn,
        General,
        Senior,
    },
    Person,
    [Age],
    [],
    |age| {
        if age.0 <= 1 {
            AgeGroupRisk::Newborn
        } else if age.0 <= 65 {
            AgeGroupRisk::General
        } else {
            AgeGroupRisk::Senior
        }
    }
);
define_multi_property!((Age, SchoolId, WorkplaceId), Person);

fn initialize_entities(context: &mut Context) {
    for person in generate_population_with_seed(10_000, 0.2, 10.0, Some(SEED)) {
        context
            .add_entity((
                Age(person.age),
                HomeId(person.home_id as u32),
                SchoolId(person.school_id as u32),
                WorkplaceId(person.workplace_id as u32),
            ))
            .unwrap();
    }
}

pub fn criterion_benchmark(criterion: &mut Criterion) {
    let mut context = Context::new();

    // Seed context RNGs for deterministic derived properties / sampling
    context.init_random(SEED);
    initialize_entities(&mut context);

    let mut criterion = criterion.benchmark_group("large_dataset");

    criterion.bench_function("bench_query_population_property_entities", |bencher| {
        bencher.iter(|| {
            black_box(context.query_entity_count((HomeId(1),)));
        });
    });

    context.index_property::<Person, HomeId>();
    criterion.bench_function(
        "bench_query_population_indexed_property_entities",
        |bencher| {
            bencher.iter(|| {
                black_box(context.query_entity_count((HomeId(1),)));
            });
        },
    );

    criterion.bench_function(
        "bench_query_population_derived_property_entities",
        |bencher| {
            bencher.iter(|| {
                black_box(context.query_entity_count((AgeGroupRisk::Senior,)));
            });
        },
    );

    // Multi-property unindexed vs indexed
    criterion.bench_function(
        "bench_query_population_multi_unindexed_entities",
        |bencher| {
            bencher.iter(|| {
                black_box(context.query_entity_count((Age(30), SchoolId(1), WorkplaceId(1))));
            });
        },
    );

    context.index_property::<Person, (Age, SchoolId, WorkplaceId)>();
    criterion.bench_function("bench_query_population_multi_indexed_entities", |bencher| {
        bencher.iter(|| {
            black_box(context.query_entity_count((Age(30), SchoolId(1), WorkplaceId(1))));
        });
    });

    {
        let entity_ids: Vec<EntityId<Person>> = context.get_entity_iterator().collect();
        let total_population = entity_ids.len();
        let mut person_idx = 0usize;
        criterion.bench_function("bench_match_entity", |bencher| {
            bencher.iter(|| {
                black_box(context.match_entity(
                    entity_ids[person_idx % total_population],
                    (Age(30u8), SchoolId(1u32), WorkplaceId(1u32)),
                ));
                person_idx += 1;
            });
        });
    }

    {
        let entity_ids: Vec<EntityId<Person>> = context.get_entity_iterator().collect();
        criterion.bench_function("bench_filter_indexed_entity", |bencher| {
            bencher.iter_batched(
                || entity_ids.clone(),
                |mut entities| {
                    context.filter_entities(
                        &mut entities,
                        (Age(30u8), SchoolId(1u32), WorkplaceId(1u32)),
                    );
                },
                BatchSize::SmallInput,
            );
        });
    }

    {
        let entity_ids: Vec<EntityId<Person>> = context.get_entity_iterator().collect();
        criterion.bench_function("bench_filter_unindexed_entity", |bencher| {
            bencher.iter_batched(
                || entity_ids.clone(),
                |mut entities| {
                    context.filter_entities(
                        &mut entities,
                        (Age(30u8), HomeId(1u32), WorkplaceId(1u32)),
                    );
                },
                BatchSize::SmallInput,
            );
        });
    }

    criterion.finish();
}

criterion_group!(example_benches, criterion_benchmark);
criterion_main!(example_benches);

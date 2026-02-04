use std::hint::black_box;

use criterion::{criterion_group, criterion_main, BatchSize, Criterion};
use ixa::context::Context;
use ixa::define_person_multi_property;
use ixa::prelude::*;
use ixa_bench::generate_population::generate_population_with_seed;
use serde::Serialize;

const SEED: u64 = 42;

// Legacy Person Properties
define_person_property!(Age, u8);
define_person_property!(HomeId, u32);
define_person_property!(SchoolId, u32);
define_person_property!(WorkplaceId, u32);

#[derive(Serialize, Copy, Clone, PartialEq, Eq, Debug, Hash)]
pub enum AgeGroupRisk {
    Newborn,
    General,
    Senior,
}

define_derived_person_property!(AgeGroupFoi, AgeGroupRisk, [Age], |age| {
    if age <= 1 {
        AgeGroupRisk::Newborn
    } else if age <= 65 {
        AgeGroupRisk::General
    } else {
        AgeGroupRisk::Senior
    }
});
define_person_multi_property!(ASW, (Age, SchoolId, WorkplaceId));

// Entity and Properties
// We use different names to avoid confusion.
define_entity!(Person);
define_property!(struct EAge(u8), Person );
define_property!(struct EHomeId(u32), Person );
define_property!(struct ESchoolId(u32), Person );
define_property!(struct EWorkplaceId(u32), Person );
define_derived_property!(
    enum EAgeGroupRisk {
        Newborn,
        General,
        Senior,
    },
    Person,
    [EAge],
    [],
    |age| {
        if age.0 <= 1 {
            EAgeGroupRisk::Newborn
        } else if age.0 <= 65 {
            EAgeGroupRisk::General
        } else {
            EAgeGroupRisk::Senior
        }
    }
);
define_multi_property!((EAge, ESchoolId, EWorkplaceId), Person);

fn initialize_people(context: &mut Context) {
    for person in generate_population_with_seed(10_000, 0.2, 10.0, Some(SEED)) {
        context
            .add_person((
                (Age, person.age),
                (HomeId, person.home_id as u32),
                (SchoolId, person.school_id as u32),
                (WorkplaceId, person.workplace_id as u32),
            ))
            .unwrap();
    }
}

fn initialize_entities(context: &mut Context) {
    for person in generate_population_with_seed(10_000, 0.2, 10.0, Some(SEED)) {
        context
            .add_entity(all!(
                Person,
                EAge(person.age),
                EHomeId(person.home_id as u32),
                ESchoolId(person.school_id as u32),
                EWorkplaceId(person.workplace_id as u32)
            ))
            .unwrap();
    }
}

pub fn criterion_benchmark(criterion: &mut Criterion) {
    let mut context = Context::new();

    // Seed context RNGs for deterministic derived properties / sampling
    context.init_random(SEED);
    initialize_people(&mut context);
    initialize_entities(&mut context);

    let mut criterion = criterion.benchmark_group("large_dataset");

    criterion.bench_function("bench_query_population_property", |bencher| {
        bencher.iter(|| {
            black_box(context.query_people_count((HomeId, 1)));
        });
    });
    criterion.bench_function("bench_query_population_property_entities", |bencher| {
        bencher.iter(|| {
            black_box(context.query_entity_count(all!(Person, EHomeId(1))));
        });
    });

    context.index_person_property(HomeId);
    criterion.bench_function("bench_query_population_indexed_property", |bencher| {
        bencher.iter(|| {
            black_box(context.query_people_count((HomeId, 1)));
        });
    });
    context.index_property::<Person, EHomeId>();
    criterion.bench_function(
        "bench_query_population_indexed_property_entities",
        |bencher| {
            bencher.iter(|| {
                black_box(context.query_entity_count(all!(Person, EHomeId(1))));
            });
        },
    );

    criterion.bench_function("bench_query_population_derived_property", |bencher| {
        bencher.iter(|| {
            black_box(context.query_people_count((AgeGroupFoi, AgeGroupRisk::Senior)));
        });
    });
    criterion.bench_function(
        "bench_query_population_derived_property_entities",
        |bencher| {
            bencher.iter(|| {
                black_box(context.query_entity_count(all!(Person, EAgeGroupRisk::Senior)));
            });
        },
    );

    // Multi-property unindexed vs indexed
    criterion.bench_function("bench_query_population_multi_unindexed", |bencher| {
        bencher.iter(|| {
            black_box(context.query_people_count((
                (Age, 30u8),
                (SchoolId, 1u32),
                (WorkplaceId, 1u32),
            )));
        });
    });
    criterion.bench_function(
        "bench_query_population_multi_unindexed_entities",
        |bencher| {
            bencher.iter(|| {
                black_box(context.query_entity_count(all!(
                    Person,
                    EAge(30),
                    ESchoolId(1),
                    EWorkplaceId(1)
                )));
            });
        },
    );

    context.index_person_property(ASW);
    criterion.bench_function("bench_query_population_multi_indexed", |bencher| {
        bencher.iter(|| {
            black_box(context.query_people_count((
                (Age, 30u8),
                (SchoolId, 1u32),
                (WorkplaceId, 1u32),
            )));
        });
    });
    context.index_property::<Person, (EAge, ESchoolId, EWorkplaceId)>();
    criterion.bench_function("bench_query_population_multi_indexed_entities", |bencher| {
        bencher.iter(|| {
            black_box(context.query_entity_count(all!(
                Person,
                EAge(30),
                ESchoolId(1),
                EWorkplaceId(1)
            )));
        });
    });

    {
        #[allow(deprecated)]
        let people_set = context.query_people(());
        let total_population = people_set.len();
        let mut person_idx = 0usize;
        criterion.bench_function("bench_match_person", |bencher| {
            bencher.iter(|| {
                black_box(context.match_person(
                    people_set[person_idx % total_population],
                    ((Age, 30u8), (SchoolId, 1u32), (WorkplaceId, 1u32)),
                ));
                person_idx += 1;
            });
        });
    }

    {
        let people_set: Vec<EntityId<Person>> = context.get_entity_iterator().collect();
        let total_population = people_set.len();
        let mut person_idx = 0usize;
        criterion.bench_function("bench_match_entity", |bencher| {
            bencher.iter(|| {
                black_box(context.match_entity(
                    people_set[person_idx % total_population],
                    all!(Person, EAge(30u8), ESchoolId(1u32), EWorkplaceId(1u32)),
                ));
                person_idx += 1;
            });
        });
    }

    {
        #[allow(deprecated)]
        let people_set = context.query_people(());
        criterion.bench_function("bench_filter_people", |bencher| {
            bencher.iter_batched(
                || people_set.clone(),
                |mut people| {
                    context.filter_people(
                        &mut people,
                        ((Age, 30u8), (SchoolId, 1u32), (WorkplaceId, 1u32)),
                    );
                },
                BatchSize::SmallInput,
            );
        });
    }

    {
        let people_set: Vec<EntityId<Person>> = context.get_entity_iterator().collect();
        criterion.bench_function("bench_filter_indexed_entity", |bencher| {
            bencher.iter_batched(
                || people_set.clone(),
                |mut people| {
                    context.filter_entities(
                        &mut people,
                        all!(Person, EAge(30u8), ESchoolId(1u32), EWorkplaceId(1u32)),
                    );
                },
                BatchSize::SmallInput,
            );
        });
    }

    {
        let people_set: Vec<EntityId<Person>> = context.get_entity_iterator().collect();
        criterion.bench_function("bench_filter_unindexed_entity", |bencher| {
            bencher.iter_batched(
                || people_set.clone(),
                |mut people| {
                    context.filter_entities(
                        &mut people,
                        all!(Person, EAge(30u8), EHomeId(1u32), EWorkplaceId(1u32)),
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

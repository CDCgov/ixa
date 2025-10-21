use std::hint::black_box;

use criterion::{criterion_group, criterion_main, Criterion};
use ixa::context::Context;
use ixa::define_multi_property;
use ixa::prelude::*;
use ixa_bench::generate_population::generate_population_with_seed;
use serde::Serialize;

const SEED: u64 = 42;

define_person_property!(Age, u8);
define_person_property!(HomeId, u32);
define_person_property!(SchoolId, u32);
define_person_property!(WorkplaceId, u32);

#[derive(Serialize, Copy, Clone, PartialEq, Eq, Debug, Hash)]
pub enum AgeGroupRisk {
    NewBorn,
    General,
    OldAdult,
}

define_derived_property!(AgeGroupFoi, AgeGroupRisk, [Age], |age| {
    if age <= 1 {
        AgeGroupRisk::NewBorn
    } else if age <= 65 {
        AgeGroupRisk::General
    } else {
        AgeGroupRisk::OldAdult
    }
});

fn initialize(context: &mut Context) {
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

fn bench_query_population_property(context: &mut Context) {
    context.query_people_count((HomeId, black_box(1)));
}

fn bench_query_population_derived_property(context: &mut Context) {
    context.query_people_count((AgeGroupFoi, black_box(AgeGroupRisk::OldAdult)));
}

pub fn criterion_benchmark(criterion: &mut Criterion) {
    define_multi_property!(ASW, (Age, SchoolId, WorkplaceId));
    let mut context = Context::new();
    // Seed context RNGs for deterministic derived properties / sampling
    context.init_random(SEED);
    initialize(&mut context);
    let mut criterion = criterion.benchmark_group("large_dataset");

    criterion.bench_function("bench_query_population_property", |bencher| {
        bencher.iter_with_large_drop(|| {
            bench_query_population_property(&mut context);
        });
    });

    context.index_property(HomeId);
    criterion.bench_function("bench_query_population_indexed_property", |bencher| {
        bencher.iter_with_large_drop(|| {
            bench_query_population_property(&mut context);
        });
    });

    criterion.bench_function("bench_query_population_derived_property", |bencher| {
        bencher.iter_with_large_drop(|| {
            bench_query_population_derived_property(&mut context);
        });
    });

    // Multi-property unindexed vs indexed
    criterion.bench_function("bench_query_population_multi_unindexed", |bencher| {
        bencher.iter_with_large_drop(|| {
            context.query_people_count((
                (Age, black_box(30u8)),
                (SchoolId, black_box(1u32)),
                (WorkplaceId, black_box(1u32)),
            ));
        });
    });
    context.index_property(ASW);
    criterion.bench_function("bench_query_population_multi_indexed", |bencher| {
        bencher.iter_with_large_drop(|| {
            context.query_people_count((
                (Age, black_box(30u8)),
                (SchoolId, black_box(1u32)),
                (WorkplaceId, black_box(1u32)),
            ));
        });
    });

    criterion.finish();
}

criterion_group!(example_benches, criterion_benchmark);
criterion_main!(example_benches);

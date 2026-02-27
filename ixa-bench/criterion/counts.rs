use std::hint::black_box;

use criterion::{criterion_group, criterion_main, Criterion};
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

define_multi_property!((Age, SchoolId, WorkplaceId), Person);

fn populate_entities(context: &mut Context, n: usize) {
    // Ensure context RNGs are deterministic as well
    context.init_random(SEED);

    for person in generate_population_with_seed(n, 0.2, 10.0, Some(SEED)) {
        let _ = context.add_entity((
            Age(person.age),
            HomeId(person.home_id as u32),
            SchoolId(person.school_id as u32),
            WorkplaceId(person.workplace_id as u32),
        ));
    }
}

pub fn criterion_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("counts");

    // moderate sized population for timing
    let mut context = Context::new();
    populate_entities(&mut context, 10_000);

    // Choose a value that likely exists
    const HOME_VAL: u32 = 1u32;

    // Unindexed single property
    group.bench_function("single_property_unindexed_entities", |bencher| {
        bencher.iter(|| {
            black_box(context.query_entity_count(black_box((HomeId(HOME_VAL),))));
        });
    });

    // Unindexed concrete + unindexed derived property
    group.bench_function("concrete_plus_derived_unindexed_entities", |bencher| {
        bencher.iter(|| {
            black_box(context.query_entity_count(black_box((HomeId(HOME_VAL), AgeGroupFoi(1)))));
        });
    });

    // Indexed single property
    context.index_property::<Person, HomeId>();
    group.bench_function("single_property_indexed_entities", |bencher| {
        bencher.iter(|| {
            black_box(context.query_entity_count(black_box((HomeId(HOME_VAL),))));
        });
    });

    // Unindexed multi-property
    group.bench_function("multi_property_unindexed_entities", |bencher| {
        bencher.iter(|| {
            black_box(context.query_entity_count(black_box((Age(30), SchoolId(1)))));
        });
    });

    // Indexed multi-property
    context.index_property::<Person, (Age, SchoolId, WorkplaceId)>();
    group.bench_function("multi_property_indexed_entities", |bencher| {
        bencher.iter(|| {
            black_box(context.query_entity_count(black_box((
                Age(30),
                SchoolId(1),
                WorkplaceId(1),
            ))));
        });
    });

    // Count indexed: measure cost of reindexing after adding new entities.
    group.bench_function("index_after_adding_entities", |bencher| {
        bencher.iter_with_setup(
            || {
                let mut ctx = Context::new();
                populate_entities(&mut ctx, 5_000);
                ctx
            },
            |mut ctx| {
                ctx.index_property::<Person, HomeId>();
                black_box(ctx.query_entity_count(black_box((HomeId(HOME_VAL),))));
            },
        );
    });

    // Reindex triggered by adding new entities: create, index, add more, then reindex by calling index_property again.
    group.bench_function("reindex_after_adding_more_entities", |bencher| {
        bencher.iter_with_setup(
            || {
                let mut ctx = Context::new();
                populate_entities(&mut ctx, 5_000);
                ctx.index_property::<Person, HomeId>();
                black_box(ctx.query_entity_count(black_box((HomeId(HOME_VAL),))));
                ctx
            },
            |mut ctx| {
                populate_entities(&mut ctx, 2_000);
                ctx.index_property::<Person, HomeId>();
                black_box(ctx.query_entity_count(black_box((HomeId(HOME_VAL),))));
            },
        );
    });

    group.finish();
}

criterion_group!(counts_benches, criterion_benchmark);
criterion_main!(counts_benches);

use std::hint::black_box;

use criterion::{criterion_group, criterion_main, Criterion};
use ixa::context::Context;
use ixa::define_person_multi_property;
use ixa::prelude::*;
use ixa_bench::generate_population::generate_population_with_seed;

const SEED: u64 = 42;

define_person_property!(Age, u8);
define_person_property!(HomeId, u32);
define_person_property!(SchoolId, u32);
define_person_property!(WorkplaceId, u32);

define_derived_person_property!(AgeGroupFoi, u8, [Age], |age| {
    if age <= 1 {
        0
    } else if age <= 65 {
        1
    } else {
        2
    }
});

define_person_multi_property!(ASW, (Age, SchoolId, WorkplaceId));

// Entity and Properties
define_entity!(Person);
define_property!(struct EAge(u8), Person);
define_property!(struct EHomeId(u32), Person);
define_property!(struct ESchoolId(u32), Person);
define_property!(struct EWorkplaceId(u32), Person);

define_derived_property!(
    struct EAgeGroupFoi(u8),
    Person,
    [EAge],
    [],
    |age| {
        if age.0 <= 1 {
            EAgeGroupFoi(0)
        } else if age.0 <= 65 {
            EAgeGroupFoi(1)
        } else {
            EAgeGroupFoi(2)
        }
    }
);

define_multi_property!((EAge, ESchoolId, EWorkplaceId), Person);

fn populate_people(context: &mut Context, n: usize) {
    // Ensure context RNGs are deterministic as well
    context.init_random(SEED);

    for person in generate_population_with_seed(n, 0.2, 10.0, Some(SEED)) {
        let _ = context.add_person((
            (Age, person.age),
            (HomeId, person.home_id as u32),
            (SchoolId, person.school_id as u32),
            (WorkplaceId, person.workplace_id as u32),
        ));
    }
}

fn populate_entities(context: &mut Context, n: usize) {
    // Ensure context RNGs are deterministic as well
    context.init_random(SEED);

    for person in generate_population_with_seed(n, 0.2, 10.0, Some(SEED)) {
        let _ = context.add_entity((
            EAge(person.age),
            EHomeId(person.home_id as u32),
            ESchoolId(person.school_id as u32),
            EWorkplaceId(person.workplace_id as u32),
        ));
    }
}

pub fn criterion_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("counts");

    // moderate sized population for timing
    let mut context = Context::new();
    populate_people(&mut context, 10_000);
    populate_entities(&mut context, 10_000);

    // Choose a value that likely exists
    const HOME_VAL: u32 = 1u32;

    // Unindexed single property
    group.bench_function("single_property_unindexed", |bencher| {
        bencher.iter(|| {
            black_box(context.query_people_count(black_box((HomeId, HOME_VAL))));
        });
    });
    group.bench_function("single_property_unindexed_entities", |bencher| {
        bencher.iter(|| {
            black_box(context.query_entity_count(black_box((EHomeId(HOME_VAL),))));
        });
    });

    // Indexed single property
    context.index_person_property(HomeId);
    group.bench_function("single_property_indexed", |bencher| {
        bencher.iter(|| {
            black_box(context.query_people_count(black_box((HomeId, HOME_VAL))));
        });
    });
    context.index_property::<Person, EHomeId>();
    group.bench_function("single_property_indexed_entities", |bencher| {
        bencher.iter(|| {
            black_box(context.query_entity_count(black_box((EHomeId(HOME_VAL),))));
        });
    });

    // Unindexed multi-property
    group.bench_function("multi_property_unindexed", |bencher| {
        bencher.iter(|| {
            black_box(context.query_people_count(black_box(((Age, 30u8), (SchoolId, 1u32)))));
        });
    });
    group.bench_function("multi_property_unindexed_entities", |bencher| {
        bencher.iter(|| {
            black_box(context.query_entity_count(black_box((EAge(30), ESchoolId(1)))));
        });
    });

    // Indexed multi-property
    context.index_person_property(ASW);
    group.bench_function("multi_property_indexed", |bencher| {
        bencher.iter(|| {
            black_box(context.query_people_count(black_box((
                (Age, 30u8),
                (SchoolId, 1u32),
                (WorkplaceId, 1u32),
            ))));
        });
    });
    context.index_property::<Person, (EAge, ESchoolId, EWorkplaceId)>();
    group.bench_function("multi_property_indexed_entities", |bencher| {
        bencher.iter(|| {
            black_box(context.query_entity_count(black_box((
                EAge(30),
                ESchoolId(1),
                EWorkplaceId(1),
            ))));
        });
    });

    // Count indexed: measure cost of reindexing after adding new people.
    group.bench_function("index_after_adding_people", |bencher| {
        bencher.iter_with_setup(
            || {
                // setup: create new context and populate, but do NOT index yet
                let mut ctx = Context::new();
                populate_people(&mut ctx, 5_000);
                ctx
            },
            |mut ctx| {
                // action: index a property (this should index existing people)
                ctx.index_person_property(HomeId);
                // touch query to ensure index is used
                black_box(ctx.query_people_count(black_box((HomeId, HOME_VAL))));
            },
        );
    });
    group.bench_function("index_after_adding_entities", |bencher| {
        bencher.iter_with_setup(
            || {
                let mut ctx = Context::new();
                populate_entities(&mut ctx, 5_000);
                ctx
            },
            |mut ctx| {
                ctx.index_property::<Person, EHomeId>();
                black_box(ctx.query_entity_count(black_box((EHomeId(HOME_VAL),))));
            },
        );
    });

    // Reindex triggered by adding new people: create, index, add more, then reindex by calling index_property again.
    group.bench_function("reindex_after_adding_more_people", |bencher| {
        bencher.iter_with_setup(
            || {
                let mut ctx = Context::new();
                populate_people(&mut ctx, 5_000);
                ctx.index_person_property(HomeId);
                // Trigger indexing for the existing people by running a query
                black_box(ctx.query_people_count(black_box((HomeId, HOME_VAL))));
                ctx
            },
            |mut ctx| {
                // Add more people (unindexed until index_unindexed_people is run)
                populate_people(&mut ctx, 2_000);
                // Re-run indexing which will pick up the new people
                ctx.index_person_property(HomeId);
                // Trigger indexing for the newly added people
                black_box(ctx.query_people_count(black_box((HomeId, HOME_VAL))));
            },
        );
    });
    group.bench_function("reindex_after_adding_more_entities", |bencher| {
        bencher.iter_with_setup(
            || {
                let mut ctx = Context::new();
                populate_entities(&mut ctx, 5_000);
                ctx.index_property::<Person, EHomeId>();
                black_box(ctx.query_entity_count(black_box((EHomeId(HOME_VAL),))));
                ctx
            },
            |mut ctx| {
                populate_entities(&mut ctx, 2_000);
                ctx.index_property::<Person, EHomeId>();
                black_box(ctx.query_entity_count(black_box((EHomeId(HOME_VAL),))));
            },
        );
    });

    group.finish();
}

criterion_group!(counts_benches, criterion_benchmark);
criterion_main!(counts_benches);

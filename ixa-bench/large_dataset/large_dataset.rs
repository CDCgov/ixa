use criterion::{criterion_group, criterion_main, Criterion};
use ixa::context::Context;
use ixa::prelude::*;
use ixa_bench::generate_population::generate_population;
use serde::{Deserialize, Serialize};
use std::hint::black_box;

define_person_property!(Age, u8);
define_person_property!(HomeId, u32);
define_person_property!(SchoolId, u32);
define_person_property!(WorkplaceId, u32);

#[derive(Deserialize, Serialize, Copy, Clone, PartialEq, Eq, Debug, Hash)]
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

#[derive(Deserialize, Debug)]
struct PeopleRecord {
    age: u8,
    #[serde(rename = "homeId")]
    home_id: u32,
    #[serde(rename = "schoolId")]
    school_id: u32,
    #[serde(rename = "workplaceId")]
    workplace_id: u32,
}

fn get_population(n: usize, schools_percent: f64, workplaces_percent: f64) -> Vec<PeopleRecord> {
    let pop = generate_population(n, schools_percent, workplaces_percent);
    pop.people
        .into_iter()
        .map(|person| PeopleRecord {
            age: person.age,
            home_id: person.home_id as u32,
            school_id: person.school_id as u32,
            workplace_id: person.workplace_id as u32,
        })
        .collect()
}

fn initialize(context: &mut Context) {
    let people = get_population(10000, 0.2, 10.0);
    for record in people {
        context
            .add_person((
                (Age, record.age),
                (HomeId, record.home_id),
                (SchoolId, record.school_id),
                (WorkplaceId, record.workplace_id),
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

pub fn criterion_benchmark(c: &mut Criterion) {
    define_multi_property_index!(Age, SchoolId, WorkplaceId);
    let mut context = Context::new();
    initialize(&mut context);

    c.bench_function("bench_query_population_property", |bencher| {
        bencher.iter_with_large_drop(|| {
            bench_query_population_property(&mut context);
        });
    });
    context.index_property(HomeId);
    c.bench_function("bench_query_population_indexed_property", |bencher| {
        bencher.iter_with_large_drop(|| {
            bench_query_population_property(&mut context);
        });
    });

    c.bench_function("bench_query_population_derived_property", |bencher| {
        bencher.iter_with_large_drop(|| {
            bench_query_population_derived_property(&mut context);
        });
    });
}

criterion_group!(example_benches, criterion_benchmark);
criterion_main!(example_benches);

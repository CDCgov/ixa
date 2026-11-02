use std::hint::black_box;

use criterion::{criterion_group, criterion_main, BatchSize, Criterion};
use ixa::prelude::*;

define_entity!(Person);
define_rng!(BenchRng);

define_network!(Contact, Person, layers = { Household, Community }, data = f64);

const PEOPLE: usize = 100_000;
const HOUSEHOLD_SIZE: usize = 4;
const COMMUNITY_DEGREE: usize = 10;

fn build(context: &mut Context) -> Vec<PersonId> {
    let people: Vec<PersonId> = (0..PEOPLE)
        .map(|_| context.add_entity(with!(Person)).unwrap())
        .collect();
    for household in people.chunks(HOUSEHOLD_SIZE) {
        for i in 0..household.len() {
            for j in (i + 1)..household.len() {
                context
                    .add_edge_bidi(Contact::Household, household[i], household[j], 1.0)
                    .unwrap();
            }
        }
    }
    // Each person gets COMMUNITY_DEGREE / 2 outgoing bidi edges to pseudo-random others.
    let mut state: u64 = 0x9E37_79B9_7F4A_7C15;
    for (i, &person) in people.iter().enumerate() {
        let mut added = 0;
        while added < COMMUNITY_DEGREE / 2 {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1);
            let j = (state >> 33) as usize % PEOPLE;
            if j == i {
                continue;
            }
            if context
                .add_edge_bidi(Contact::Community, person, people[j], 0.1)
                .is_ok()
            {
                added += 1;
            }
        }
    }
    people
}

fn setup() -> (Context, Vec<PersonId>) {
    let mut context = Context::new();
    context.init_random(7);
    let people = build(&mut context);
    (context, people)
}

fn bench_network(c: &mut Criterion) {
    let mut group = c.benchmark_group("network");
    group.sample_size(20);

    group.bench_function("build_100k", |b| {
        b.iter_batched(
            || {
                let mut context = Context::new();
                context.init_random(7);
                context
            },
            |mut context| {
                black_box(build(&mut context));
                context
            },
            BatchSize::PerIteration,
        );
    });

    let (context, people) = setup();

    group.bench_function("iterate_all_layers_100k", |b| {
        b.iter(|| {
            let mut total = 0.0;
            for &person in &people {
                for &layer in Contact::ALL {
                    for edge in context.get_edges(layer, person) {
                        total += edge.data;
                    }
                }
            }
            black_box(total)
        });
    });

    group.bench_function("get_edge_hit_100k", |b| {
        b.iter(|| {
            let mut hits = 0;
            for household in people.chunks(HOUSEHOLD_SIZE) {
                if household.len() == HOUSEHOLD_SIZE
                    && context
                        .get_edge(Contact::Household, household[0], household[3])
                        .is_some()
                {
                    hits += 1;
                }
            }
            black_box(hits)
        });
    });

    group.bench_function("degree_100k", |b| {
        b.iter(|| {
            let mut total = 0;
            for &person in &people {
                total += context.get_degree(Contact::Community, person);
            }
            black_box(total)
        });
    });

    group.bench_function("sample_edge_weighted_100k", |b| {
        b.iter(|| {
            let mut picked = 0;
            for &person in &people {
                if context
                    .sample_edge_weighted(BenchRng, Contact::Community, person, |e| e.data)
                    .is_some()
                {
                    picked += 1;
                }
            }
            black_box(picked)
        });
    });

    group.bench_function("sample_edge_100k", |b| {
        b.iter(|| {
            let mut picked = 0;
            for &person in &people {
                if context
                    .sample_edge(BenchRng, Contact::Community, person)
                    .is_some()
                {
                    picked += 1;
                }
            }
            black_box(picked)
        });
    });

    group.finish();
}

criterion_group!(benches, bench_network);
criterion_main!(benches);

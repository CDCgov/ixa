use criterion::{criterion_group, criterion_main, Criterion};
use ixa::prelude::*;
use ixa::{define_multi_property, PersonId};
use std::hint::black_box;

define_rng!(IndexBenchRng);

define_person_property!(Property10, u8, |context: &Context, _person: PersonId| {
    context.sample_range(IndexBenchRng, 0..10)
});
define_person_property!(Property100, u8, |context: &Context, _person: PersonId| {
    context.sample_range(IndexBenchRng, 0..100)
});

define_person_property!(MProperty10, u8, |context: &Context, _person: PersonId| {
    context.sample_range(IndexBenchRng, 0..10)
});
define_person_property!(MProperty100, u8, |context: &Context, _person: PersonId| {
    context.sample_range(IndexBenchRng, 0..100)
});

define_multi_property!(MProperty, (MProperty10, MProperty100));

pub fn criterion_benchmark(c: &mut Criterion) {
    let mut context = Context::new();
    for _ in 0..100000 {
        let _ = context.add_person(());
    }

    let mut numbers: Vec<u8> = Vec::with_capacity(1000);
    for _ in 0..1000 {
        numbers.push(context.sample_range(IndexBenchRng, 0..100));
    }

    context.index_property(Property10);
    context.index_property(Property100);
    context.index_property(MProperty);

    c.bench_function("query_single_indexed_property", |bencher| {
        bencher.iter(|| {
            for number in &numbers {
                black_box(context.query_people_count(black_box((Property10, number % 10))));
            }
        });
    });

    c.bench_function(
        "query_multiple_individually_indexed_properties",
        |bencher| {
            bencher.iter(|| {
                for number in &numbers {
                    black_box(context.query_people_count(black_box((
                        (Property10, number * 3 % 10),
                        (Property100, *number),
                    ))));
                }
            });
        },
    );

    c.bench_function("query_indexed_multi-property", |bencher| {
        bencher.iter(|| {
            for number in &numbers {
                black_box(
                    context.query_people_count(black_box((MProperty, (number * 3 % 10, *number)))),
                );
            }
        });
    });
}

criterion_group!(index_benches, criterion_benchmark);
criterion_main!(index_benches);

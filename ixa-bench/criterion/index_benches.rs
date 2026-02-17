use std::hint::black_box;

use criterion::{criterion_group, criterion_main, Criterion};
use ixa::define_multi_property;
use ixa::prelude::*;

define_rng!(IndexBenchRng);

// Entity and Properties
define_entity!(Person);
define_property!(struct Property10(u8), Person);
define_property!(struct Property100(u8), Person);
define_property!(struct MultiProperty10(u8), Person);
define_property!(struct MultiProperty100(u8), Person);
define_multi_property!((MultiProperty10, MultiProperty100), Person);

pub fn criterion_benchmark(criterion: &mut Criterion) {
    let mut criterion = criterion.benchmark_group("indexing");

    let mut context = Context::new();
    for _ in 0..100_000 {
        context
            .add_entity((
                Property10(context.sample_range(IndexBenchRng, 0..10)),
                Property100(context.sample_range(IndexBenchRng, 0..100)),
                MultiProperty10(context.sample_range(IndexBenchRng, 0..10)),
                MultiProperty100(context.sample_range(IndexBenchRng, 0..100)),
            ))
            .unwrap();
    }

    let mut numbers: Vec<u8> = Vec::with_capacity(1000);
    for _ in 0..1000 {
        numbers.push(context.sample_range(IndexBenchRng, 0..100));
    }

    context.index_property::<Person, Property10>();
    context.index_property::<Person, Property100>();
    context.index_property::<Person, (MultiProperty10, MultiProperty100)>(); // Jointly indexed, but not its components

    criterion.bench_function(
        "with_query_results_single_indexed_property_entities",
        |bencher| {
            bencher.iter(|| {
                for number in &numbers {
                    context.with_query_results(
                        black_box((Property10(number % 10),)),
                        &mut |entity_ids| {
                            black_box(entity_ids.try_len());
                        },
                    );
                }
            });
        },
    );

    criterion.bench_function(
        "with_query_results_multiple_individually_indexed_properties_entities",
        |bencher| {
            bencher.iter(|| {
                for number in &numbers {
                    context.with_query_results(
                        black_box((
                            Property10(number.wrapping_mul(3) % 10),
                            Property100(*number),
                        )),
                        &mut |entity_ids| {
                            black_box(entity_ids.try_len());
                        },
                    );
                }
            });
        },
    );

    criterion.bench_function(
        "with_query_results_indexed_multi-property_entities",
        |bencher| {
            bencher.iter(|| {
                for number in &numbers {
                    context.with_query_results(
                        // We are using the fact that a query detects when it is equivalent to a multi-property.
                        black_box((
                            MultiProperty10(number.wrapping_mul(3) % 10),
                            MultiProperty100(*number),
                        )),
                        &mut |entity_ids| {
                            black_box(entity_ids.try_len());
                        },
                    );
                }
            });
        },
    );

    criterion.bench_function(
        "query_people_count_single_indexed_property_entities",
        |bencher| {
            bencher.iter(|| {
                for number in &numbers {
                    black_box(context.query_entity_count(black_box((Property10(number % 10),))));
                }
            });
        },
    );

    criterion.bench_function(
        "query_people_count_multiple_individually_indexed_properties_entities",
        |bencher| {
            bencher.iter(|| {
                for number in &numbers {
                    black_box(context.query_entity_count(black_box((
                        Property10(number.wrapping_mul(3) % 10),
                        Property100(*number),
                    ))));
                }
            });
        },
    );

    criterion.bench_function(
        "query_people_count_indexed_multi-property_entities",
        |bencher| {
            bencher.iter(|| {
                for number in &numbers {
                    black_box(
                        // We are using the fact that a query detects when it is equivalent to a multi-property.
                        context.query_entity_count(black_box((
                            MultiProperty10(number.wrapping_mul(3) % 10),
                            MultiProperty100(*number),
                        ))),
                    );
                }
            });
        },
    );

    criterion.bench_function("query_people_single_indexed_property_entities", |bencher| {
        bencher.iter(|| {
            for number in &numbers {
                // There is no exact equivalent to `Context::query_people`.
                let result_iter =
                    black_box(context.query_result_iterator(black_box((Property10(number % 10),))));
                black_box(result_iter.collect::<Vec<_>>());
            }
        });
    });

    criterion.bench_function(
        "query_people_multiple_individually_indexed_properties_entities",
        |bencher| {
            bencher.iter(|| {
                for number in &numbers {
                    // There is no exact equivalent to `Context::query_people`.
                    let result_iter = black_box(context.query_result_iterator(black_box((
                        Property10(number % 10),
                        Property100(*number),
                    ))));
                    black_box(result_iter.collect::<Vec<_>>());
                }
            });
        },
    );

    criterion.bench_function("query_people_indexed_multi-property_entities", |bencher| {
        bencher.iter(|| {
            for number in &numbers {
                // There is no exact equivalent to `Context::query_people`.
                // We are using the fact that a query detects when it is equivalent to a multi-property.
                let result_iter = black_box(context.query_result_iterator(black_box((
                    MultiProperty10(number % 10),
                    MultiProperty100(*number),
                ))));
                black_box(result_iter.collect::<Vec<_>>());
            }
        });
    });

    criterion.finish();
}

criterion_group!(index_benches, criterion_benchmark);
criterion_main!(index_benches);

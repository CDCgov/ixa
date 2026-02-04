use std::hint::black_box;

use criterion::{criterion_group, criterion_main, Criterion};
use ixa::prelude::*;
use ixa::{define_multi_property, define_person_multi_property, PersonId};

define_rng!(IndexBenchRng);

// Legacy Person Properties
define_person_property!(Property10, u8, |context: &Context, _person: PersonId| {
    context.sample_range(IndexBenchRng, 0..10)
});
define_person_property!(Property100, u8, |context: &Context, _person: PersonId| {
    context.sample_range(IndexBenchRng, 0..100)
});
// Multi-property components
define_person_property!(MProperty10, u8, |context: &Context, _person: PersonId| {
    context.sample_range(IndexBenchRng, 0..10)
});
define_person_property!(MProperty100, u8, |context: &Context, _person: PersonId| {
    context.sample_range(IndexBenchRng, 0..100)
});
// Multi-property
define_person_multi_property!(MProperty, (MProperty10, MProperty100));

// Entity and Properties
define_entity!(Animal);
define_property!(struct AProperty10(u8), Animal);
define_property!(struct AProperty100(u8), Animal);
define_property!(struct AMProperty10(u8), Animal);
define_property!(struct AMProperty100(u8), Animal);
define_multi_property!((AMProperty10, AMProperty100), Animal);

pub fn criterion_benchmark(criterion: &mut Criterion) {
    let mut criterion = criterion.benchmark_group("indexing");

    let mut context = Context::new();
    for _ in 0..100_000 {
        context.add_person(()).unwrap();
        // An entity's properties cannot be computed dynamically in the same way as Person Properties.
        context
            .add_entity(all!(
                Animal,
                AProperty10(context.sample_range(IndexBenchRng, 0..10)),
                AProperty100(context.sample_range(IndexBenchRng, 0..100)),
                AMProperty10(context.sample_range(IndexBenchRng, 0..10)),
                AMProperty100(context.sample_range(IndexBenchRng, 0..100))
            ))
            .unwrap();
    }

    let mut numbers: Vec<u8> = Vec::with_capacity(1000);
    for _ in 0..1000 {
        numbers.push(context.sample_range(IndexBenchRng, 0..100));
    }

    context.index_person_property(Property10);
    context.index_person_property(Property100);
    context.index_person_property(MProperty);
    context.index_property::<Animal, AProperty10>();
    context.index_property::<Animal, AProperty100>();
    context.index_property::<Animal, (AMProperty10, AMProperty100)>(); // Jointly indexed, but not its components

    criterion.bench_function("with_query_results_single_indexed_property", |bencher| {
        bencher.iter(|| {
            for number in &numbers {
                context.with_query_people_results(
                    black_box((Property10, number % 10)),
                    &mut |people_set| {
                        black_box(people_set.len());
                    },
                );
            }
        });
    });
    criterion.bench_function(
        "with_query_results_single_indexed_property_entities",
        |bencher| {
            bencher.iter(|| {
                for number in &numbers {
                    context.with_query_results(
                        black_box(all!(Animal, AProperty10(number % 10))),
                        &mut |people_set| {
                            black_box(people_set.len());
                        },
                    );
                }
            });
        },
    );

    criterion.bench_function(
        "with_query_results_multiple_individually_indexed_properties",
        |bencher| {
            bencher.iter(|| {
                for number in &numbers {
                    context.with_query_people_results(
                        black_box((
                            (Property10, number.wrapping_mul(3) % 10),
                            (Property100, *number),
                        )),
                        &mut |people_set| {
                            black_box(people_set.len());
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
                        black_box(all!(
                            Animal,
                            AProperty10(number.wrapping_mul(3) % 10),
                            AProperty100(*number)
                        )),
                        &mut |people_set| {
                            black_box(people_set.len());
                        },
                    );
                }
            });
        },
    );

    criterion.bench_function("with_query_results_indexed_multi-property", |bencher| {
        bencher.iter(|| {
            for number in &numbers {
                context.with_query_people_results(
                    black_box((MProperty, (number.wrapping_mul(3) % 10, *number))),
                    &mut |people_set| {
                        black_box(people_set.len());
                    },
                );
            }
        });
    });
    criterion.bench_function(
        "with_query_results_indexed_multi-property_entities",
        |bencher| {
            bencher.iter(|| {
                for number in &numbers {
                    context.with_query_results(
                        // We are using the fact that a query detects when it is equivalent to a multi-property.
                        black_box(all!(
                            Animal,
                            AMProperty10(number.wrapping_mul(3) % 10),
                            AMProperty100(*number)
                        )),
                        &mut |people_set| {
                            black_box(people_set.len());
                        },
                    );
                }
            });
        },
    );

    criterion.bench_function("query_people_count_single_indexed_property", |bencher| {
        bencher.iter(|| {
            for number in &numbers {
                black_box(context.query_people_count(black_box((Property10, number % 10))));
            }
        });
    });
    criterion.bench_function(
        "query_people_count_single_indexed_property_entities",
        |bencher| {
            bencher.iter(|| {
                for number in &numbers {
                    black_box(
                        context
                            .query_entity_count(black_box(all!(Animal, AProperty10(number % 10)))),
                    );
                }
            });
        },
    );

    criterion.bench_function(
        "query_people_count_multiple_individually_indexed_properties",
        |bencher| {
            bencher.iter(|| {
                for number in &numbers {
                    black_box(context.query_people_count(black_box((
                        (Property10, number.wrapping_mul(3) % 10),
                        (Property100, *number),
                    ))));
                }
            });
        },
    );
    criterion.bench_function(
        "query_people_count_multiple_individually_indexed_properties_entities",
        |bencher| {
            bencher.iter(|| {
                for number in &numbers {
                    black_box(context.query_entity_count(black_box(all!(
                        Animal,
                        AProperty10(number.wrapping_mul(3) % 10),
                        AProperty100(*number)
                    ))));
                }
            });
        },
    );

    criterion.bench_function("query_people_count_indexed_multi-property", |bencher| {
        bencher.iter(|| {
            for number in &numbers {
                black_box(context.query_people_count(black_box((
                    MProperty,
                    (number.wrapping_mul(3) % 10, *number),
                ))));
            }
        });
    });
    criterion.bench_function(
        "query_people_count_indexed_multi-property_entities",
        |bencher| {
            bencher.iter(|| {
                for number in &numbers {
                    black_box(
                        // We are using the fact that a query detects when it is equivalent to a multi-property.
                        context.query_entity_count(black_box(all!(
                            Animal,
                            AMProperty10(number.wrapping_mul(3) % 10),
                            AMProperty100(*number)
                        ))),
                    );
                }
            });
        },
    );

    criterion.bench_function("query_people_single_indexed_property", |bencher| {
        bencher.iter(|| {
            for number in &numbers {
                #[allow(deprecated)]
                black_box(context.query_people(black_box((Property10, number % 10))));
            }
        });
    });
    criterion.bench_function("query_people_single_indexed_property_entities", |bencher| {
        bencher.iter(|| {
            for number in &numbers {
                // There is no exact equivalent to `Context::query_people`.
                let result_iter = black_box(
                    context
                        .query_result_iterator(black_box(all!(Animal, AProperty10(number % 10)))),
                );
                black_box(result_iter.collect::<Vec<_>>());
            }
        });
    });

    criterion.bench_function(
        "query_people_multiple_individually_indexed_properties",
        |bencher| {
            bencher.iter(|| {
                for number in &numbers {
                    #[allow(deprecated)]
                    black_box(context.query_people(black_box((
                        (Property10, number.wrapping_mul(3) % 10),
                        (Property100, *number),
                    ))));
                }
            });
        },
    );
    criterion.bench_function(
        "query_people_multiple_individually_indexed_properties_entities",
        |bencher| {
            bencher.iter(|| {
                for number in &numbers {
                    // There is no exact equivalent to `Context::query_people`.
                    let result_iter = black_box(context.query_result_iterator(black_box(all!(
                        Animal,
                        AProperty10(number % 10),
                        AProperty100(*number)
                    ))));
                    black_box(result_iter.collect::<Vec<_>>());
                }
            });
        },
    );

    criterion.bench_function("query_people_indexed_multi-property", |bencher| {
        bencher.iter(|| {
            for number in &numbers {
                #[allow(deprecated)]
                black_box(context.query_people(black_box((
                    MProperty,
                    (number.wrapping_mul(3) % 10, *number),
                ))));
            }
        });
    });
    criterion.bench_function("query_people_indexed_multi-property_entities", |bencher| {
        bencher.iter(|| {
            for number in &numbers {
                // There is no exact equivalent to `Context::query_people`.
                // We are using the fact that a query detects when it is equivalent to a multi-property.
                let result_iter = black_box(context.query_result_iterator(black_box(all!(
                    Animal,
                    AMProperty10(number % 10),
                    AMProperty100(*number)
                ))));
                black_box(result_iter.collect::<Vec<_>>());
            }
        });
    });

    criterion.finish();
}

criterion_group!(index_benches, criterion_benchmark);
criterion_main!(index_benches);

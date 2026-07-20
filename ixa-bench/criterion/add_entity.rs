use std::hint::black_box;

use criterion::{criterion_group, criterion_main, BatchSize, Criterion, Throughput};
use ixa::prelude::*;

const ENTITY_COUNT: usize = 100_000;

define_entity!(WideEntity);
define_entity!(NarrowEntity);

define_property!(
    struct TargetValue(u8),
    WideEntity,
    default_const = TargetValue(0)
);
define_property!(
    struct AuxiliaryValueOne(u8),
    WideEntity,
    default_const = AuxiliaryValueOne(0)
);
define_property!(
    struct AuxiliaryValueTwo(u8),
    WideEntity,
    default_const = AuxiliaryValueTwo(0)
);
define_property!(
    struct AdditionalValueOne(u8),
    WideEntity,
    default_const = AdditionalValueOne(0)
);
define_property!(
    struct AdditionalValueTwo(u8),
    WideEntity,
    default_const = AdditionalValueTwo(0)
);
define_property!(
    struct AdditionalValueThree(u8),
    WideEntity,
    default_const = AdditionalValueThree(0)
);
define_property!(
    struct AdditionalValueFour(u8),
    WideEntity,
    default_const = AdditionalValueFour(0)
);
define_property!(
    struct AdditionalValueFive(u8),
    WideEntity,
    default_const = AdditionalValueFive(0)
);

impl_property!(TargetValue, NarrowEntity, default_const = TargetValue(0));
impl_property!(
    AuxiliaryValueOne,
    NarrowEntity,
    default_const = AuxiliaryValueOne(0)
);
impl_property!(
    AuxiliaryValueTwo,
    NarrowEntity,
    default_const = AuxiliaryValueTwo(0)
);

fn initialize_context_for_no_indexes_explicit_nondefault_wide() -> Context {
    let mut context = Context::new();
    context
        .add_entity(with!(
            WideEntity,
            TargetValue(1),
            AuxiliaryValueOne(1),
            AuxiliaryValueTwo(1),
        ))
        .unwrap();
    context
}

fn initialize_context_for_one_full_index_explicit_nondefault_wide() -> Context {
    let mut context = Context::new();
    context.index_property::<WideEntity, TargetValue>();
    context
        .add_entity(with!(
            WideEntity,
            TargetValue(1),
            AuxiliaryValueOne(1),
            AuxiliaryValueTwo(1),
        ))
        .unwrap();
    context
}

fn initialize_context_for_three_full_indexes_explicit_nondefault_wide() -> Context {
    let mut context = Context::new();
    context.index_property::<WideEntity, TargetValue>();
    context.index_property::<WideEntity, AuxiliaryValueOne>();
    context.index_property::<WideEntity, AuxiliaryValueTwo>();
    context
        .add_entity(with!(
            WideEntity,
            TargetValue(1),
            AuxiliaryValueOne(1),
            AuxiliaryValueTwo(1),
        ))
        .unwrap();
    context
}

fn initialize_context_for_one_value_count_index_explicit_nondefault_wide() -> Context {
    let mut context = Context::new();
    context.index_property_counts::<WideEntity, TargetValue>();
    context
        .add_entity(with!(
            WideEntity,
            TargetValue(1),
            AuxiliaryValueOne(1),
            AuxiliaryValueTwo(1),
        ))
        .unwrap();
    context
}

fn initialize_context_for_one_full_index_omitted_default_wide() -> Context {
    let mut context = Context::new();
    context.index_property::<WideEntity, TargetValue>();
    context
        .add_entity(with!(
            WideEntity,
            AuxiliaryValueOne(1),
            AuxiliaryValueTwo(1),
        ))
        .unwrap();
    context
}

fn initialize_context_for_one_full_index_explicit_default_wide() -> Context {
    let mut context = Context::new();
    context.index_property::<WideEntity, TargetValue>();
    context
        .add_entity(with!(
            WideEntity,
            TargetValue(0),
            AuxiliaryValueOne(1),
            AuxiliaryValueTwo(1),
        ))
        .unwrap();
    context
}

fn initialize_context_for_one_full_index_explicit_nondefault_narrow() -> Context {
    let mut context = Context::new();
    context.index_property::<NarrowEntity, TargetValue>();
    context
        .add_entity(with!(
            NarrowEntity,
            TargetValue(1),
            AuxiliaryValueOne(1),
            AuxiliaryValueTwo(1),
        ))
        .unwrap();
    context
}

fn criterion_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("add_entity");
    group.throughput(Throughput::Elements(ENTITY_COUNT as u64));

    group.bench_function("no_indexes_explicit_nondefault_wide", |bencher| {
        bencher.iter_batched_ref(
            initialize_context_for_no_indexes_explicit_nondefault_wide,
            |context| {
                for _ in 0..ENTITY_COUNT {
                    black_box(
                        context
                            .add_entity(with!(
                                WideEntity,
                                TargetValue(1),
                                AuxiliaryValueOne(1),
                                AuxiliaryValueTwo(1),
                            ))
                            .unwrap(),
                    );
                }
            },
            BatchSize::PerIteration,
        );
    });

    group.bench_function("one_full_index_explicit_nondefault_wide", |bencher| {
        bencher.iter_batched_ref(
            initialize_context_for_one_full_index_explicit_nondefault_wide,
            |context| {
                for _ in 0..ENTITY_COUNT {
                    black_box(
                        context
                            .add_entity(with!(
                                WideEntity,
                                TargetValue(1),
                                AuxiliaryValueOne(1),
                                AuxiliaryValueTwo(1),
                            ))
                            .unwrap(),
                    );
                }
            },
            BatchSize::PerIteration,
        );
    });

    group.bench_function("three_full_indexes_explicit_nondefault_wide", |bencher| {
        bencher.iter_batched_ref(
            initialize_context_for_three_full_indexes_explicit_nondefault_wide,
            |context| {
                for _ in 0..ENTITY_COUNT {
                    black_box(
                        context
                            .add_entity(with!(
                                WideEntity,
                                TargetValue(1),
                                AuxiliaryValueOne(1),
                                AuxiliaryValueTwo(1),
                            ))
                            .unwrap(),
                    );
                }
            },
            BatchSize::PerIteration,
        );
    });

    group.bench_function(
        "one_value_count_index_explicit_nondefault_wide",
        |bencher| {
            bencher.iter_batched_ref(
                initialize_context_for_one_value_count_index_explicit_nondefault_wide,
                |context| {
                    for _ in 0..ENTITY_COUNT {
                        black_box(
                            context
                                .add_entity(with!(
                                    WideEntity,
                                    TargetValue(1),
                                    AuxiliaryValueOne(1),
                                    AuxiliaryValueTwo(1),
                                ))
                                .unwrap(),
                        );
                    }
                },
                BatchSize::PerIteration,
            );
        },
    );

    group.bench_function("one_full_index_omitted_default_wide", |bencher| {
        bencher.iter_batched_ref(
            initialize_context_for_one_full_index_omitted_default_wide,
            |context| {
                for _ in 0..ENTITY_COUNT {
                    black_box(
                        context
                            .add_entity(with!(
                                WideEntity,
                                AuxiliaryValueOne(1),
                                AuxiliaryValueTwo(1),
                            ))
                            .unwrap(),
                    );
                }
            },
            BatchSize::PerIteration,
        );
    });

    group.bench_function("one_full_index_explicit_default_wide", |bencher| {
        bencher.iter_batched_ref(
            initialize_context_for_one_full_index_explicit_default_wide,
            |context| {
                for _ in 0..ENTITY_COUNT {
                    black_box(
                        context
                            .add_entity(with!(
                                WideEntity,
                                TargetValue(0),
                                AuxiliaryValueOne(1),
                                AuxiliaryValueTwo(1),
                            ))
                            .unwrap(),
                    );
                }
            },
            BatchSize::PerIteration,
        );
    });

    group.bench_function("one_full_index_explicit_nondefault_narrow", |bencher| {
        bencher.iter_batched_ref(
            initialize_context_for_one_full_index_explicit_nondefault_narrow,
            |context| {
                for _ in 0..ENTITY_COUNT {
                    black_box(
                        context
                            .add_entity(with!(
                                NarrowEntity,
                                TargetValue(1),
                                AuxiliaryValueOne(1),
                                AuxiliaryValueTwo(1),
                            ))
                            .unwrap(),
                    );
                }
            },
            BatchSize::PerIteration,
        );
    });

    group.finish();
}

criterion_group!(add_entity_benches, criterion_benchmark);
criterion_main!(add_entity_benches);

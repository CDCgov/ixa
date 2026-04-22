use std::hash::{Hash, Hasher};
use std::hint::black_box;

use criterion::{criterion_group, criterion_main, BatchSize, Criterion};
use ixa::hashing::one_shot_128;
use ixa::prelude::*;
use serde::Serialize;

define_entity!(Person);

define_property!(struct HashScalar(u8), Person, default_const = HashScalar(0));
define_property!(
    struct HashStruct {
        bucket: u16,
        code: u32,
    },
    Person,
    default_const = HashStruct { bucket: 0, code: 0 }
);
define_property!(struct HashFloat(f64), Person, impl_eq_hash = both, default_const = HashFloat(0.0));
define_property!(struct MultiHashByte(u8), Person, default_const = MultiHashByte(0));
define_property!(struct MultiHashFloat(f64), Person, impl_eq_hash = both, default_const = MultiHashFloat(0.0));
define_multi_property!((MultiHashByte, MultiHashFloat), Person);

define_property!(struct FloatQueryValue(f64), Person, impl_eq_hash = both, default_const = FloatQueryValue(0.0));

#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy, Serialize)]
struct CounterBucket(pub u8);

impl_property!(CounterBucket, Person, default_const = CounterBucket(0));

#[derive(Debug, Clone, Copy, Serialize)]
struct FloatCounterValue(pub f64);

impl PartialEq for FloatCounterValue {
    fn eq(&self, other: &Self) -> bool {
        self.0.to_bits() == other.0.to_bits()
    }
}

impl Eq for FloatCounterValue {}

impl Hash for FloatCounterValue {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.to_bits().hash(state);
    }
}

impl_property!(
    FloatCounterValue,
    Person,
    default_const = FloatCounterValue(0.0)
);

fn hash_benchmarks(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("property_semantics_hashing");

    let scalar = HashScalar(42);
    group.bench_function("scalar_property_hash", |bencher| {
        bencher.iter(|| black_box(one_shot_128(&HashScalar::make_canonical(black_box(scalar)))));
    });

    let structured = HashStruct {
        bucket: 17,
        code: 12_345,
    };
    group.bench_function("struct_property_hash", |bencher| {
        bencher.iter(|| {
            black_box(one_shot_128(&HashStruct::make_canonical(black_box(
                structured,
            ))))
        });
    });

    let float_value = HashFloat(1234.5);
    group.bench_function("float_property_hash", |bencher| {
        bencher.iter(|| {
            black_box(one_shot_128(&HashFloat::make_canonical(black_box(
                float_value,
            ))))
        });
    });

    let multi_value = (MultiHashByte(7), MultiHashFloat(3.5));
    let multi_canonical =
        <(MultiHashByte, MultiHashFloat) as Property<Person>>::make_canonical(multi_value);
    group.bench_function("multi_property_hash", |bencher| {
        bencher.iter(|| black_box(one_shot_128(black_box(&multi_canonical))));
    });

    group.bench_function("raw_one_shot_hash_scalar", |bencher| {
        bencher.iter(|| black_box(one_shot_128(black_box(&42u8))));
    });

    group.finish();
}

fn build_float_query_context() -> Context {
    let mut context = Context::new();
    for i in 0..100_000 {
        let value = ((i % 2048) as f64) * 0.25;
        context.add_entity((FloatQueryValue(value),)).unwrap();
    }
    context.index_property::<Person, FloatQueryValue>();
    context
}

fn float_query_benchmarks(criterion: &mut Criterion) {
    let context = build_float_query_context();
    let probes: Vec<f64> = (0..1000).map(|i| ((i % 2048) as f64) * 0.25).collect();

    let mut group = criterion.benchmark_group("property_semantics_float_queries");

    group.bench_function("float_query_entity_count_indexed", |bencher| {
        bencher.iter(|| {
            for probe in &probes {
                black_box(context.query_entity_count(black_box((FloatQueryValue(*probe),))));
            }
        });
    });

    group.bench_function("float_query_with_query_results_indexed", |bencher| {
        bencher.iter(|| {
            for probe in &probes {
                context.with_query_results(
                    black_box((FloatQueryValue(*probe),)),
                    &mut |entity_ids| {
                        black_box(entity_ids.try_len());
                    },
                );
            }
        });
    });

    group.finish();
}

fn build_value_change_counter_context() -> Context {
    let mut context = Context::new();
    let mut entities = Vec::with_capacity(10_000);

    for i in 0..10_000 {
        let entity_id = context
            .add_entity((
                CounterBucket((i % 32) as u8),
                FloatCounterValue(((i % 256) as f64) * 0.5),
            ))
            .unwrap();
        entities.push(entity_id);
    }

    context.track_periodic_value_change_counts::<Person, (CounterBucket,), FloatCounterValue, _>(
        1.0,
        |_context, counter| {
            black_box(counter.iter().count());
        },
    );

    let first_wave = entities.clone();
    context.add_plan(0.1, move |context| {
        for (i, entity_id) in first_wave.iter().copied().enumerate() {
            context.set_property(entity_id, FloatCounterValue((((i + 1) % 256) as f64) * 0.5));
        }
    });

    context.add_plan(0.2, Context::shutdown);

    context
}

fn value_change_counter_benchmarks(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("property_semantics_value_change_counts");

    group.bench_function("float_value_change_counter_execute", |bencher| {
        bencher.iter_batched(
            build_value_change_counter_context,
            |mut context| {
                context.execute();
                black_box(context.get_current_time());
            },
            BatchSize::LargeInput,
        );
    });

    group.finish();
}

fn criterion_benchmark(criterion: &mut Criterion) {
    hash_benchmarks(criterion);
    float_query_benchmarks(criterion);
    value_change_counter_benchmarks(criterion);
}

criterion_group!(property_semantics_benches, criterion_benchmark);
criterion_main!(property_semantics_benches);

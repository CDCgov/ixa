use std::cell::RefCell;
use std::hint::black_box;

use criterion::{criterion_group, criterion_main, BatchSize, Criterion};
use ixa::prelude::*;
use ixa::{impl_derived_property, impl_property};

define_entity!(Person);

define_property!(struct IndependentValue(u64), Person, default_const = IndependentValue(0));
define_property!(struct BaseValue(u64), Person, default_const = BaseValue(0));

#[derive(Debug, PartialEq, Eq, Clone, Copy, serde::Serialize, Hash)]
struct MixedBaseValue(u64);

impl_property!(MixedBaseValue, Person, default_const = MixedBaseValue(0));

define_derived_property!(
    struct DerivedLowBit(bool),
    Person,
    [BaseValue],
    [],
    |base| {
        let base: BaseValue = base;
        DerivedLowBit(base.0 & 1 == 1)
    }
);

define_derived_property!(
    struct DerivedSecondBit(bool),
    Person,
    [BaseValue],
    [],
    |base| {
        let base: BaseValue = base;
        DerivedSecondBit(base.0 & 2 == 2)
    }
);

define_derived_property!(
    struct DerivedBucket(u8),
    Person,
    [BaseValue],
    [],
    |base| {
        let base: BaseValue = base;
        DerivedBucket((base.0 % 3) as u8)
    }
);

define_derived_property!(
    struct MixedDerivedIndexed(bool),
    Person,
    [MixedBaseValue],
    [],
    |base| {
        let base: MixedBaseValue = base;
        MixedDerivedIndexed(base.0 & 1 == 0)
    }
);

#[derive(Debug, PartialEq, Eq, Clone, Copy, serde::Serialize, Hash)]
struct MixedDerivedCounted(u8);

impl_derived_property!(MixedDerivedCounted, Person, [MixedBaseValue], [], |base| {
    let base: MixedBaseValue = base;
    MixedDerivedCounted((base.0 % 4) as u8)
});

define_derived_property!(
    struct MixedDerivedHandled(bool),
    Person,
    [MixedBaseValue],
    [],
    |base| {
        let base: MixedBaseValue = base;
        MixedDerivedHandled(base.0.trailing_zeros() == 0)
    }
);

pub fn criterion_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("set_property");

    group.bench_function("set_property_no_dependents", |bencher| {
        let mut context = Context::new();
        let person = context.add_entity((IndependentValue(0),)).unwrap();
        let mut next_value = 0u64;

        bencher.iter(|| {
            next_value = next_value.wrapping_add(1);
            context.set_property(black_box(person), IndependentValue(black_box(next_value)));
        });
    });

    group.bench_function("set_property_three_dependents", |bencher| {
        let mut context = Context::new();
        let person = context.add_entity((BaseValue(0),)).unwrap();
        let mut next_value = 0u64;

        bencher.iter(|| {
            next_value = next_value.wrapping_add(1);
            context.set_property(black_box(person), BaseValue(black_box(next_value)));
        });
    });

    group.bench_function("set_property_three_dependents_mixed", |bencher| {
        let mut context = Context::new();
        let person = context
            .add_entity((MixedBaseValue(0), IndependentValue(0)))
            .unwrap();

        context.index_property::<Person, MixedDerivedIndexed>();
        context
            .track_periodic_value_change_counts::<Person, (MixedBaseValue, ), MixedDerivedCounted, _>(
                1.0,
                |_context, _counter| {},
            );
        context.subscribe_to_event(
            |_context, _event: PropertyChangeEvent<Person, MixedDerivedHandled>| {},
        );

        // Set a value to trigger lazy initialization of the derived properties.
        context.set_property(person, MixedBaseValue(42));
        context.execute();
        let context = RefCell::new(context);

        // Reuse one Context so lazy initialization is amortized, but flush callbacks between
        // measured chunks to avoid pathological callback queue growth.
        bencher.iter_batched(
            || {
                context.borrow_mut().execute();
            },
            |_| {
                let mut context = context.borrow_mut();
                for value in 0..256 {
                    context.set_property(black_box(person), MixedBaseValue(black_box(value)));
                }
            },
            BatchSize::SmallInput,
        );
    });

    group.finish();
}

criterion_group!(set_property_benches, criterion_benchmark);
criterion_main!(set_property_benches);

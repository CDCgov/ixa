use std::hint::black_box;

use criterion::{criterion_group, criterion_main, Criterion};
use ixa::prelude::*;

define_entity!(Person);

define_property!(struct IndependentValue(u64), Person, default_const = IndependentValue(0));
define_property!(struct BaseValue(u64), Person, default_const = BaseValue(0));

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

    group.finish();
}

criterion_group!(set_property_benches, criterion_benchmark);
criterion_main!(set_property_benches);

use std::hint::black_box;

use criterion::{criterion_group, criterion_main, BatchSize, Criterion};
use ixa::context::Context;
use ixa::prelude::*;

const N_PEOPLE: usize = 10_000;

define_entity!(Person);
define_property!(struct Age(u8), Person);
define_property!(struct Prop1(u32), Person, default_const = Prop1(0));
define_property!(struct Prop2(u32), Person, default_const = Prop2(0));
define_property!(struct Prop3(u32), Person, default_const = Prop3(0));
define_property!(struct Prop4(u32), Person, default_const = Prop4(0));
define_property!(struct Prop5(u32), Person, default_const = Prop5(0));
define_property!(struct Prop6(u32), Person, default_const = Prop6(0));
define_property!(struct Prop7(u32), Person, default_const = Prop7(0));
define_property!(struct Prop8(u32), Person, default_const = Prop8(0));
define_property!(struct Prop9(u32), Person, default_const = Prop9(0));

pub fn criterion_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("bulk_add_people");

    group.bench_function("add_10000_one_by_one", |bencher| {
        bencher.iter_batched(
            Context::new,
            |mut context| {
                let mut ids = Vec::with_capacity(N_PEOPLE);
                for i in 0..N_PEOPLE {
                    let i_u32 = i as u32;
                    ids.push(
                        context
                            .add_entity((
                                Age((i % 100) as u8),
                                Prop1(i_u32 + 1),
                                Prop2((i_u32 % 10_000) + 1),
                                Prop3((i_u32 % 100_000) + 1),
                                Prop4((i_u32 % 1_000_000) + 1),
                                Prop5((i_u32.wrapping_mul(3)) + 1),
                                Prop6((i_u32.wrapping_mul(5)) + 1),
                                Prop7((i_u32.wrapping_mul(7)) + 1),
                                Prop8((i_u32.wrapping_mul(11)) + 1),
                                Prop9((i_u32.wrapping_mul(13)) + 1),
                            ))
                            .unwrap(),
                    );
                }
                black_box(context.get_entity_count::<Person>());
                black_box(ids.len());
            },
            BatchSize::SmallInput,
        );
    });

    group.bench_function("add_10000_in_bulk", |bencher| {
        bencher.iter_batched(
            Context::new,
            |mut context| {
                let ids = context
                    .add_entities::<Person, _, _>((0..N_PEOPLE).map(|i| {
                        let i_u32 = i as u32;
                        (
                            Age((i % 100) as u8),
                            Prop1(i_u32 + 1),
                            Prop2((i_u32 % 10_000) + 1),
                            Prop3((i_u32 % 100_000) + 1),
                            Prop4((i_u32 % 1_000_000) + 1),
                            Prop5((i_u32.wrapping_mul(3)) + 1),
                            Prop6((i_u32.wrapping_mul(5)) + 1),
                            Prop7((i_u32.wrapping_mul(7)) + 1),
                            Prop8((i_u32.wrapping_mul(11)) + 1),
                            Prop9((i_u32.wrapping_mul(13)) + 1),
                        )
                    }))
                    .unwrap();
                black_box(ids.len());
            },
            BatchSize::SmallInput,
        );
    });

    group.finish();
}

criterion_group!(bulk_add_people_benches, criterion_benchmark);
criterion_main!(bulk_add_people_benches);

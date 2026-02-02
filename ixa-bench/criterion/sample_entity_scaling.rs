use std::hint::black_box;

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use ixa::prelude::*;

// This benchmark checks for regressions that sample_entity is O(1)
// This is a very common use case in a model, i.e., "get the next person to infect"
// Which tends to happen at every time step, so it's very important that it's fast

define_rng!(SampleScalingRng);

const SEED: u64 = 42;

// Entity and Properties
define_entity!(Mosquito);
define_property!(struct Species(u8), Mosquito);
define_property!(struct Region(u8), Mosquito);
define_multi_property!((Species, Region), Mosquito);

/// Population sizes to test for O(n) scaling analysis
const POPULATION_SIZES: [usize; 3] = [1_000, 10_000, 100_000];

fn setup_context(population_size: usize) -> Context {
    let mut context = Context::new();
    context.init_random(SEED);

    // Index single property
    context.index_property::<Mosquito, Species>();

    // Index multi-property
    context.index_property::<Mosquito, (Species, Region)>();

    // Add population with random property values
    for _ in 0..population_size {
        context
            .add_entity((
                Species(context.sample_range(SampleScalingRng, 0..10)),
                Region(context.sample_range(SampleScalingRng, 0..10)),
            ))
            .unwrap();
    }

    context
}

pub fn bench_sample_entity_whole_population(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("sample_entity_whole_population");

    for &size in &POPULATION_SIZES {
        let context = setup_context(size);

        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |bencher, _| {
            bencher.iter(|| {
                let _: Option<EntityId<Mosquito>> = black_box({
                    black_box(());
                    context.sample_entity(SampleScalingRng, ())
                });
            });
        });
    }

    group.finish();
}

pub fn bench_sample_entity_single_property_indexed(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("sample_entity_single_property_indexed");

    for &size in &POPULATION_SIZES {
        let context = setup_context(size);

        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |bencher, _| {
            bencher.iter(|| {
                // Query a single indexed property - should be O(1) due to known length
                black_box(context.sample_entity(SampleScalingRng, black_box((Species(5),))))
            });
        });
    }

    group.finish();
}

pub fn bench_sample_entity_multi_property_indexed(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("sample_entity_multi_property_indexed");

    for &size in &POPULATION_SIZES {
        let context = setup_context(size);

        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |bencher, _| {
            bencher.iter(|| {
                // Query a multi-property index - should be O(1) due to known length
                black_box(
                    context.sample_entity(SampleScalingRng, black_box((Species(5), Region(3)))),
                )
            });
        });
    }

    group.finish();
}

criterion_group!(
    sample_entity_scaling,
    bench_sample_entity_whole_population,
    bench_sample_entity_single_property_indexed,
    bench_sample_entity_multi_property_indexed,
);
criterion_main!(sample_entity_scaling);

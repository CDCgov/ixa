use std::collections::BTreeMap;
use std::hint::black_box;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use ixa::prelude::*;

define_rng!(SampleScalingRng);

const SEED: u64 = 42;
define_entity!(Mosquito);
define_property!(struct Species(u8), Mosquito);
define_property!(struct Region(u8), Mosquito);
define_multi_property!((Species, Region), Mosquito);

const POPULATION_SIZES: [usize; 3] = [1_000, 10_000, 100_000];

// Shared place to stash "ns per sample" per (bench_name, size)
type Results = Arc<Mutex<BTreeMap<(String, usize), f64>>>;

fn setup_context(population_size: usize) -> Context {
    let mut context = Context::new();
    context.init_random(SEED);

    context.index_property::<Mosquito, Species>();
    context.index_property::<Mosquito, (Species, Region)>();

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

/// Print a scaling summary for one benchmark "family"
fn print_scaling_summary(results: &BTreeMap<(String, usize), f64>, bench_name: &str) {
    let mut points: Vec<(usize, f64)> = results
        .iter()
        .filter_map(|((name, n), t)| {
            if name == bench_name {
                Some((*n, *t))
            } else {
                None
            }
        })
        .collect();

    points.sort_by_key(|(n, _)| *n);
    if points.len() < 2 {
        return;
    }

    let (n0, t0) = points[0];

    eprintln!("\n=== Scaling summary: {bench_name} ===");
    eprintln!("  baseline: n={n0}, t={t0:.2} ns/sample");
    eprintln!("  ratios vs baseline:");
    for (n, t) in &points {
        eprintln!("    n={n:>7}: {t:>10.2} ns/sample  (x{:.3})", t / t0);
    }
}

// This is so we can do an O_n analysis of the benchmark
fn bench_ns_per_sample<F>(bencher: &mut criterion::Bencher, mut f: F) -> f64
where
    F: FnMut(),
{
    let mut ns_per_sample: f64 = 0.0;

    bencher.iter_custom(|iters| {
        let start = Instant::now();
        for _ in 0..iters {
            f();
            black_box(());
        }
        let elapsed = start.elapsed();

        // record ns/sample for this measurement batch
        ns_per_sample = elapsed.as_secs_f64() * 1e9 / (iters as f64);

        elapsed
    });

    ns_per_sample
}

pub fn bench_sample_entity_whole_population(c: &mut Criterion, results: Results) {
    let bench_name = "sample_entity_whole_population";
    let mut group = c.benchmark_group(bench_name);

    for &size in &POPULATION_SIZES {
        let context = setup_context(size);

        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, _| {
            let ns = bench_ns_per_sample(b, || {
                let _: Option<EntityId<Mosquito>> =
                    context.sample_entity(SampleScalingRng, mosquito![]);
            });

            results
                .lock()
                .unwrap()
                .insert((bench_name.to_string(), size), ns);
        });
    }

    group.finish();
}

pub fn bench_sample_entity_single_property_indexed(c: &mut Criterion, results: Results) {
    let bench_name = "sample_entity_single_property_indexed";
    let mut group = c.benchmark_group(bench_name);

    for &size in &POPULATION_SIZES {
        let context = setup_context(size);

        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, _| {
            let ns = bench_ns_per_sample(b, || {
                let _ = context.sample_entity(SampleScalingRng, mosquito![Species(5)]);
            });

            results
                .lock()
                .unwrap()
                .insert((bench_name.to_string(), size), ns);
        });
    }

    group.finish();
}

pub fn bench_sample_entity_multi_property_indexed(c: &mut Criterion, results: Results) {
    let bench_name = "sample_entity_multi_property_indexed";
    let mut group = c.benchmark_group(bench_name);

    for &size in &POPULATION_SIZES {
        let context = setup_context(size);

        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, _| {
            let ns = bench_ns_per_sample(b, || {
                let _ = context.sample_entity(SampleScalingRng, mosquito![Species(5), Region(3)]);
            });

            results
                .lock()
                .unwrap()
                .insert((bench_name.to_string(), size), ns);
        });
    }

    group.finish();
}

fn sample_entity_scaling(c: &mut Criterion) {
    let results: Results = Arc::new(Mutex::new(BTreeMap::new()));

    bench_sample_entity_whole_population(c, results.clone());
    bench_sample_entity_single_property_indexed(c, results.clone());
    bench_sample_entity_multi_property_indexed(c, results.clone());

    // Prints a scaling summary at the end like:
    //   === Scaling summary: sample_entity_whole_population ===
    //   baseline: n=1000, t=789.94 ns/sample
    //   ratios vs baseline:
    //     n=   1000:     789.94 ns/sample  (x1.000)
    //     n=  10000:    7675.09 ns/sample  (x9.716)
    //     n= 100000:   77670.29 ns/sample  (x98.325)
    let results = results.lock().unwrap();
    print_scaling_summary(&results, "sample_entity_whole_population");
    print_scaling_summary(&results, "sample_entity_single_property_indexed");
    print_scaling_summary(&results, "sample_entity_multi_property_indexed");
}

criterion_group!(benches, sample_entity_scaling);
criterion_main!(benches);

//! Benchmarks for `ixa::random::sample_single_excluding`.
//!
//! Compares the two sampling strategies (rejection sampling vs. linear scan)
//! across a range of slice sizes, with a single occurrence of `excluded` in
//! the slice (the typical "sample a peer, but not me" pattern).
//!
//! The output `=== Strategy comparison ===` summary shows the crossover point
//! that the implementation's small-slice threshold should target.

use std::collections::BTreeMap;
use std::hint::black_box;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use ixa::rand::rngs::StdRng;
use ixa::rand::SeedableRng;
use ixa::random::{
    sample_single_excluding, sample_single_excluding_iteration, sample_single_excluding_rejection,
};

const SEED: u64 = 42;

/// Slice sizes to test. Focused on the small-n regime where the strategy
/// crossover happens — at larger n rejection wins by orders of magnitude.
const SLICE_SIZES: [usize; 3] = [2, 3, 4];

type Results = Arc<Mutex<BTreeMap<(String, usize), f64>>>;

/// Builds a Vec<u32> with values 0..n, and returns it together with one
/// "excluded" value that is present exactly once in the slice (mid-range).
fn build_slice(n: usize) -> (Vec<u32>, u32) {
    let data: Vec<u32> = (0..n as u32).collect();
    let excluded = (n / 2) as u32;
    (data, excluded)
}

fn bench_ns_per_sample<F: FnMut()>(b: &mut criterion::Bencher, mut f: F) -> f64 {
    let mut ns_per_sample = 0.0_f64;
    b.iter_custom(|iters| {
        let start = Instant::now();
        for _ in 0..iters {
            f();
            black_box(());
        }
        let elapsed = start.elapsed();
        ns_per_sample = elapsed.as_secs_f64() * 1e9 / (iters as f64);
        elapsed
    });
    ns_per_sample
}

fn record(results: &Results, name: &str, n: usize, ns: f64) {
    results.lock().unwrap().insert((name.to_string(), n), ns);
}

fn bench_strategies(c: &mut Criterion, results: &Results) {
    let mut group = c.benchmark_group("sample_single_excluding");
    group.sample_size(50);

    for &n in &SLICE_SIZES {
        let (slice, excluded) = build_slice(n);
        let id_param = format!("n={n:>7}");

        group.bench_with_input(BenchmarkId::new("rejection", &id_param), &(), |b, _| {
            let mut rng = StdRng::seed_from_u64(SEED);
            let ns = bench_ns_per_sample(b, || {
                let v = sample_single_excluding_rejection(
                    black_box(&mut rng),
                    black_box(&slice),
                    excluded,
                )
                .unwrap();
                black_box(v);
            });
            record(results, "rejection", n, ns);
        });

        group.bench_with_input(BenchmarkId::new("iteration", &id_param), &(), |b, _| {
            let mut rng = StdRng::seed_from_u64(SEED);
            let ns = bench_ns_per_sample(b, || {
                let v = sample_single_excluding_iteration(
                    black_box(&mut rng),
                    black_box(&slice),
                    excluded,
                )
                .unwrap();
                black_box(v);
            });
            record(results, "iteration", n, ns);
        });

        group.bench_with_input(BenchmarkId::new("auto", &id_param), &(), |b, _| {
            let mut rng = StdRng::seed_from_u64(SEED);
            let ns = bench_ns_per_sample(b, || {
                let v = sample_single_excluding(black_box(&mut rng), black_box(&slice), excluded)
                    .unwrap();
                black_box(v);
            });
            record(results, "auto", n, ns);
        });
    }

    group.finish();
}

fn print_summary(results: &BTreeMap<(String, usize), f64>) {
    eprintln!("\n=== Strategy comparison: sample_single_excluding (ns/sample) ===");
    eprintln!(
        "{:>10}  {:>12}  {:>12}  {:>12}  {:>10}",
        "n", "rejection", "iteration", "auto", "winner"
    );

    for &n in &SLICE_SIZES {
        let r = results[&("rejection".to_string(), n)];
        let i = results[&("iteration".to_string(), n)];
        let a = results[&("auto".to_string(), n)];
        let winner = if r < i { "rejection" } else { "iteration" };
        eprintln!("{n:>10}  {r:>12.1}  {i:>12.1}  {a:>12.1}  {winner:>10}");
    }
}

fn benches(c: &mut Criterion) {
    let results: Results = Arc::new(Mutex::new(BTreeMap::new()));
    bench_strategies(c, &results);
    print_summary(&results.lock().unwrap());
}

criterion_group!(group, benches);
criterion_main!(group);

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

type Measurements = BTreeMap<(&'static str, usize), f64>;
type Results = Arc<Mutex<Measurements>>;

#[derive(Debug, PartialEq)]
struct SummaryRow {
    size: usize,
    rejection: f64,
    iteration: f64,
    automatic: f64,
}

fn complete_rows(results: &Measurements, slice_sizes: &[usize]) -> Vec<SummaryRow> {
    slice_sizes
        .iter()
        .filter_map(|&size| {
            Some(SummaryRow {
                size,
                rejection: *results.get(&("rejection", size))?,
                iteration: *results.get(&("iteration", size))?,
                automatic: *results.get(&("auto", size))?,
            })
        })
        .collect()
}

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

fn record(results: &Results, name: &'static str, n: usize, ns: f64) {
    results.lock().unwrap().insert((name, n), ns);
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

fn print_summary(results: &Measurements) {
    let rows = complete_rows(results, &SLICE_SIZES);
    if rows.is_empty() {
        return;
    }

    eprintln!("\n=== Strategy comparison: sample_single_excluding (ns/sample) ===");
    eprintln!(
        "{:>10}  {:>12}  {:>12}  {:>12}  {:>10}",
        "n", "rejection", "iteration", "auto", "winner"
    );

    for SummaryRow {
        size,
        rejection,
        iteration,
        automatic,
    } in rows
    {
        let winner = if rejection < iteration {
            "rejection"
        } else {
            "iteration"
        };
        eprintln!(
            "{size:>10}  {rejection:>12.1}  {iteration:>12.1}  {automatic:>12.1}  {winner:>10}"
        );
    }
}

fn benches(c: &mut Criterion) {
    let results: Results = Arc::new(Mutex::new(Measurements::new()));
    bench_strategies(c, &results);
    print_summary(&results.lock().unwrap());
}

criterion_group!(group, benches);
criterion_main!(group);

#[cfg(test)]
mod tests {
    #[test]
    fn no_rows_are_returned_without_measurements() {
        assert!(super::complete_rows(&super::Measurements::new(), &super::SLICE_SIZES).is_empty());
    }

    #[test]
    fn no_rows_are_returned_for_a_single_strategy() {
        let results = super::Measurements::from([
            (("rejection", 2), 2.1),
            (("rejection", 3), 3.1),
            (("rejection", 4), 4.1),
        ]);

        assert!(super::complete_rows(&results, &super::SLICE_SIZES).is_empty());
    }

    #[test]
    fn only_complete_rows_are_returned() {
        let results = super::Measurements::from([
            (("rejection", 2), 2.1),
            (("iteration", 2), 2.2),
            (("rejection", 3), 3.1),
            (("iteration", 3), 3.2),
            (("auto", 3), 3.3),
        ]);

        assert_eq!(
            super::complete_rows(&results, &super::SLICE_SIZES),
            vec![super::SummaryRow {
                size: 3,
                rejection: 3.1,
                iteration: 3.2,
                automatic: 3.3,
            }]
        );
    }
}

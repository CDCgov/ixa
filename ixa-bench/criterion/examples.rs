use criterion::{criterion_group, criterion_main, Criterion};
use ixa::prelude::*;
use ixa_example_basic_infection::initialize as basic_infection_initialize;
use ixa_example_births_deaths::initialize as births_deaths_initialize;
use tempfile::tempdir;

pub fn criterion_benchmark(criterion: &mut Criterion) {
    let output_dir = tempdir().unwrap();

    let mut criterion = criterion.benchmark_group("examples");
    criterion.bench_function("example-basic-infection", |bencher| {
        bencher.iter_with_large_drop(|| {
            let mut context = Context::new();
            basic_infection_initialize(&mut context);
            context.execute();
            context
        });
    });

    criterion.bench_function("example-births-deaths", |bencher| {
        bencher.iter_with_large_drop(|| {
            let mut context = Context::new();
            births_deaths_initialize(&mut context, output_dir.path());
            context.execute();
            context
        });
    });

    criterion.finish()
}

criterion_group!(example_benches, criterion_benchmark);
criterion_main!(example_benches);

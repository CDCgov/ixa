use criterion::{criterion_group, criterion_main, Criterion};
use ixa::prelude::*;
use ixa_example_births_deaths::initialize;
use tempfile::tempdir;

pub fn criterion_benchmark(c: &mut Criterion) {
    let output_dir = tempdir().unwrap();

    c.bench_function("example births-deaths", |bencher| {
        bencher.iter_with_large_drop(|| {
            let mut context = Context::new();
            initialize(&mut context, output_dir.path());
            context.execute();
            context
        });
    });
}

criterion_group!(example_benches, criterion_benchmark);
criterion_main!(example_benches);

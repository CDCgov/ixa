use criterion::{criterion_group, criterion_main, Criterion};
use ixa::Context;
use ixa_example_births_deaths::initialize;
use std::fs;
use std::path::Path;

pub fn criterion_benchmark(c: &mut Criterion) {
    let parameters_path = fs::canonicalize(Path::new("."))
        .unwrap()
        .join("../examples/births-deaths/input.json");
    c.bench_function("example births-deaths", |bencher| {
        bencher.iter_with_large_drop(|| {
            let mut context = Context::new();
            initialize(&mut context, &parameters_path);
            context.execute();
            context
        });
    });
}

criterion_group!(example_benches, criterion_benchmark);
criterion_main!(example_benches);

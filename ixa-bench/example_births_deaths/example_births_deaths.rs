use criterion::{criterion_group, criterion_main, Criterion};
use ixa::prelude::*;
use ixa_example_births_deaths::initialize;
use std::fs;
use std::path::Path;
use tempfile::tempdir;

pub fn criterion_benchmark(c: &mut Criterion) {
    let parameters_path =
        fs::canonicalize(Path::new(".").join("../examples/births-deaths/input.json")).unwrap();
    let output_dir = tempdir().unwrap();
    println!("nothing...");

    c.bench_function("example births-deaths", |bencher| {
        bencher.iter_with_large_drop(|| {
            let mut context = Context::new();
            initialize(&mut context, parameters_path.as_path(), output_dir.path());
            context.execute();
            context
        });
    });
}

criterion_group!(example_benches, criterion_benchmark);
criterion_main!(example_benches);

use criterion::{criterion_group, criterion_main, Criterion};
use ixa_example_births_deaths::initialize;

pub fn criterion_benchmark(c: &mut Criterion) {
    c.bench_function("example births-deaths", |bencher| {
        bencher.iter_with_large_drop(|| {
            let mut context = initialize();
            context.execute();
            context
        });
    });
}

criterion_group!(example_benches, criterion_benchmark);
criterion_main!(example_benches);

use criterion::{criterion_group, criterion_main, Criterion};
use ixa::prelude::*;
use ixa_example_basic_infection::initialize;

pub fn criterion_benchmark(criterion: &mut Criterion) {
    let mut criterion = criterion.benchmark_group("example");
    criterion.bench_function("example basic-infection", |bencher| {
        bencher.iter_with_large_drop(|| {
            let mut context = Context::new();
            initialize(&mut context);
            context.execute();
            context
        });
    });
}

criterion_group!(example_benches, criterion_benchmark);
criterion_main!(example_benches);

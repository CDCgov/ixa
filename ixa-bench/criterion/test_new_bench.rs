use criterion::{criterion_group, criterion_main, Criterion};

pub fn criterion_benchmark(c: &mut Criterion) {
    c.bench_function("example births-deaths", |bencher| {
        bencher.iter(|| 1 + 2);
    });
}

criterion_group!(test_bench, criterion_benchmark);
criterion_main!(test_bench);

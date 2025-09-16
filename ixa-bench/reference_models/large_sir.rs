use std::hint::black_box;

use criterion::{criterion_group, criterion_main, Criterion};
use ixa_bench::reference_sir::{sir_baseline, sir_ixa, ParametersBuilder};

pub fn large_sir(c: &mut Criterion) {
    let mut group = c.benchmark_group("large_sir");

    let parameters = ParametersBuilder::default()
        .population(10_000)
        .initial_infections(1000)
        .max_time(10.0)
        .build()
        .unwrap();

    group.bench_function("baseline", |b| {
        b.iter(|| {
            let mut model = sir_baseline::Model::new(black_box(parameters.clone()));
            model.run();
        })
    });

    group.bench_function("ixa_with_queries", |b| {
        b.iter(|| {
            let mut model = sir_ixa::Model::new(
                black_box(parameters.clone()),
                sir_ixa::ModelOptions::default(),
            );
            model.run();
        })
    });

    group.bench_function("ixa_no_queries", |b| {
        b.iter(|| {
            let mut model = sir_ixa::Model::new(
                black_box(parameters.clone()),
                sir_ixa::ModelOptions {
                    queries_enabled: false,
                },
            );
            model.run();
        })
    });

    group.finish();
}

fn configure_criterion() -> Criterion {
    Criterion::default().measurement_time(std::time::Duration::from_secs(20))
}

criterion_group! {
    name = benches;
    config = configure_criterion();
    targets = large_sir
}
criterion_main!(benches);

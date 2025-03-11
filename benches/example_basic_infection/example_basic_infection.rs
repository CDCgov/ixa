pub mod incidence_report;
pub mod infection_manager;
pub mod people;
pub mod transmission_manager;

use criterion::{criterion_group, criterion_main, Criterion};
use ixa::{Context, ContextRandomExt};

static POPULATION: u64 = 1000;
static SEED: u64 = 123;
static MAX_TIME: f64 = 303.0;
static FOI: f64 = 0.1;
static INFECTION_DURATION: f64 = 5.0;

pub fn initialize(context: &mut Context) {
    context.init_random(SEED);

    people::init(context);
    transmission_manager::init(context);
    infection_manager::init(context);
    incidence_report::init(context).unwrap_or_else(|e| {
        eprintln!("failed to init incidence_report: {e}");
    });
    context.add_plan(MAX_TIME, |context| {
        context.shutdown();
    });
}

pub fn criterion_benchmark(c: &mut Criterion) {
    c.bench_function("example basic-infection", |bencher| {
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

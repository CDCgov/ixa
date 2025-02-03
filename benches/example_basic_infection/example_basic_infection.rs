use criterion::{criterion_group, criterion_main, Criterion};
use ixa::context::Context;
use ixa::random::ContextRandomExt;

use ixa_example_basic_infection::{
    incidence_report, infection_manager, people, transmission_manager,
};

static SEED: u64 = 123;
static MAX_TIME: f64 = 303.0;

fn basic_infection() -> Context {
    let mut context = Context::new();

    context.init_random(SEED);

    people::init(&mut context);
    transmission_manager::init(&mut context);
    infection_manager::init(&mut context);
    incidence_report::init(&mut context).expect("failed to init incidence report");

    context.add_plan(MAX_TIME, |context| {
        context.shutdown();
    });

    context.execute();

    context
}

pub fn criterion_benchmark(c: &mut Criterion) {
    c.bench_function("example basic-infection", |bencher| {
        bencher.iter_with_large_drop(basic_infection);
    });
}

criterion_group!(example_benches, criterion_benchmark);
criterion_main!(example_benches);

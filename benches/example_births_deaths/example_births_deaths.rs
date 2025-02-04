use criterion::{criterion_group, criterion_main, Criterion};
use ixa::{Context, ContextGlobalPropertiesExt, ContextRandomExt};
use std::path::Path;

use ixa_example_births_deaths::{
    demographics_report, incidence_report, infection_manager, parameters_loader,
    population_manager, transmission_manager,
};

fn births_deaths() -> Context {
    let mut context = Context::new();
    let current_dir = Path::new(file!()).parent().unwrap();
    let file_path = current_dir.join("input.json");

    parameters_loader::init_parameters(&mut context, &file_path)
        .expect("failed to load parameters");

    let parameters = context
        .get_global_property_value(parameters_loader::Parameters)
        .unwrap()
        .clone();
    context.init_random(parameters.seed);

    demographics_report::init(&mut context).expect("failed to init demographics report");
    incidence_report::init(&mut context).expect("failed to init incidence report");

    population_manager::init(&mut context);
    transmission_manager::init(&mut context);
    infection_manager::init(&mut context);

    context.add_plan(parameters.max_time, |context| {
        context.shutdown();
    });

    context.execute();
    context
}

pub fn criterion_benchmark(c: &mut Criterion) {
    c.bench_function("example births-deaths", |bencher| {
        bencher.iter_with_large_drop(births_deaths);
    });
}

criterion_group!(example_benches, criterion_benchmark);
criterion_main!(example_benches);

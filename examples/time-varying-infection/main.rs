use ixa::context::Context;
use ixa::random::ContextRandomExt;

mod exposure_manager;
mod incidence_report;
mod infection_manager;
mod population_loader;

static POPULATION: u64 = 1000;
static SEED: u64 = 123;
static MAX_TIME: f64 = 303.0;
static FOI: f64 = 0.1;
static INFECTION_DURATION: f64 = 5.0;

fn main() {
    let mut context = Context::new();

    context.init_random(SEED);

    population_loader::init(&mut context);
    exposure_manager::init(&mut context);
    infection_manager::init(&mut context);
    incidence_report::init(&mut context);

    context.add_plan(MAX_TIME, |context| {
        context.shutdown();
    });

    context.execute();
}

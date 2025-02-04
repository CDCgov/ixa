use ixa::context::Context;
use ixa::error::IxaError;
use ixa::random::ContextRandomExt;
use ixa::run_with_args;

mod incidence_report;
mod infection_manager;
mod people;
mod transmission_manager;

static POPULATION: u64 = 1000;
static SEED: u64 = 123;
static MAX_TIME: f64 = 303.0;
static FOI: f64 = 0.1;
static INFECTION_DURATION: f64 = 5.0;

fn initialize(context: &mut Context) -> Result<(), IxaError> {
    context.init_random(SEED);

    people::init(context);
    transmission_manager::init(context);
    infection_manager::init(context);
    incidence_report::init(context)?;

    context.add_plan(MAX_TIME, |context| {
        context.shutdown();
    });
    Ok(())
}

fn main() {
    run_with_args(|ctx, _, _| initialize(ctx)).expect("failed to run the model");
}

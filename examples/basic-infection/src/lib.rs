use ixa::context::Context;
use ixa::error::IxaError;
use ixa::random::ContextRandomExt;

pub mod incidence_report;
pub mod infection_manager;
pub mod people;
pub mod transmission_manager;

static POPULATION: u64 = 1000;
static SEED: u64 = 123;
static MAX_TIME: f64 = 303.0;
static FOI: f64 = 0.1;
static INFECTION_DURATION: f64 = 5.0;

pub fn initialize() -> Result<Context, IxaError> {
    let mut context = Context::new();

    context.init_random(SEED);

    people::init(&mut context);
    transmission_manager::init(&mut context);
    infection_manager::init(&mut context);
    incidence_report::init(&mut context)?;

    context.add_plan(MAX_TIME, |context| {
        context.shutdown();
    });
    Ok(context)
}

use ixa::context::Context;
use ixa::error::IxaError;
use ixa::random::ContextRandomExt;

mod incidence_report;
mod infection_manager;
mod people;
mod transmission_manager;

use crate::people::ContextPeopleExt;

static POPULATION: u64 = 1000;
static SEED: u64 = 123;
static MAX_TIME: f64 = 303.0;
static FOI: f64 = 0.1;
static INFECTION_DURATION: f64 = 5.0;

fn init() -> Result<Context, IxaError> {
    let mut context = Context::new();

    context.init_random(SEED);

    for _ in 0..POPULATION {
        context.create_person();
    }

    transmission_manager::init(&mut context);
    infection_manager::init(&mut context);
    incidence_report::init(&mut context)?;

    context.add_plan(MAX_TIME, |context| {
        context.shutdown();
    });

    Ok(context)
}

fn main() {
    let init_result = init();
    match init_result {
        Ok(mut context) => {
            context.execute();
            println!("Simulation completed successfully");
        }
        Err(ixa_error) => {
            println!("Initialization failure: {ixa_error}");
        }
    }
}

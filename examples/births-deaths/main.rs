use ixa::error::IxaError;
use ixa::random::ContextRandomExt;
use ixa::{context::Context, global_properties::ContextGlobalPropertiesExt};
use std::path::Path;

mod demographics_report;
mod incidence_report;
mod infection_manager;
mod parameters_loader;
mod population_manager;
mod transmission_manager;

use crate::parameters_loader::Parameters;

fn initialize() -> Result<Context, IxaError> {
    let mut context = Context::new();
    let current_dir = Path::new(file!()).parent().unwrap();
    let file_path = current_dir.join("input.json");

    parameters_loader::init_parameters(&mut context, &file_path)?;

    let parameters = context
        .get_global_property_value(Parameters)
        .unwrap()
        .clone();
    context.init_random(parameters.seed);

    demographics_report::init(&mut context)?;
    incidence_report::init(&mut context)?;

    population_manager::init(&mut context);
    transmission_manager::init(&mut context);
    infection_manager::init(&mut context);

    context.add_plan(parameters.max_time, |context| {
        context.shutdown();
    });
    println!("{parameters:?}");
    Ok(context)
}

fn main() {
    let mut context = initialize().expect("Could not initialize context.");

    context.execute();
}

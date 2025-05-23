use ixa::prelude::*;
use std::path::Path;

mod incidence_report;
mod infection_manager;
mod parameters_loader;
mod transmission_manager;

use crate::parameters_loader::Parameters;

use serde::{Deserialize, Serialize};

#[derive(Debug, Hash, Eq, PartialEq, Clone, Copy, Serialize, Deserialize)]
pub enum InfectionStatusValue {
    S,
    I,
    R,
}
define_person_property_with_default!(
    InfectionStatus,
    InfectionStatusValue,
    InfectionStatusValue::S
);

fn initialize() -> Result<Context, IxaError> {
    let mut context = Context::new();
    let file_path = Path::new("examples")
        .join("parameter-loading")
        .join("input.json");

    parameters_loader::init_parameters(&mut context, &file_path)?;

    let parameters = context
        .get_global_property_value(Parameters)
        .unwrap()
        .clone();
    context.init_random(parameters.seed);

    for _ in 0..parameters.population {
        context.add_person(()).unwrap();
    }

    transmission_manager::init(&mut context);
    infection_manager::init(&mut context);
    incidence_report::init(&mut context)?;

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

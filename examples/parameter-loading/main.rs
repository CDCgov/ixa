use ixa::error::IxaError;
use ixa::global_properties::IxaParams;
use ixa::people::ContextPeopleExt;
use ixa::random::ContextRandomExt;
use ixa::{
    context::Context, define_person_property_with_default,
    global_properties::ContextGlobalPropertiesExt,
};
use ixa_derive::IxaParams;
use std::path::{Path, PathBuf};

mod incidence_report;
mod infection_manager;
mod parameters_loader;
mod transmission_manager;

use crate::parameters_loader::Parameters;

use clap::Parser;
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

#[derive(IxaParams, Debug, Serialize, Deserialize, Parser)]
#[command(version, about)]
pub struct Params {
    /// The number of infections we seed the population with.
    pub initial_infections: usize,
    /// The maximum run time of the simulation; even if there are still infections
    /// scheduled to occur, the simulation will stop at this time.
    pub max_time: f64,
    /// The random seed for the simulation.
    pub seed: u64,
    /// A constant rate of infection applied to all individuals.
    pub rate_of_infection: f64,
    /// The duration of the infection in days
    pub infection_duration: f64,
    /// The period at which to report tabulated values
    pub report_period: f64,
    /// The path to the synthetic population file loaded in `population_loader`
    pub synth_population_file: PathBuf,
}

fn main() {
    let test =
        Params::parse_config_and_args(std::path::Path::new("examples/parameter-loading/ixa.json"))
            .unwrap();
    println!("{:?}", test);
    let mut context = initialize().expect("Could not initialize context.");

    context.execute();
}

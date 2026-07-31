use ixa::prelude::*;
use ixa::runner::run_with_args;
mod incidence_report;
mod loader;
mod network;
mod parameters;
mod seir;
use std::path::PathBuf;

use parameters::Parameters;

define_entity!(Person);
define_rng!(MainRng);

fn main() -> Result<(), Box<dyn std::error::Error>> {
    run_with_args(|context, _, _| {
        initialize(context)?;
        Ok(())
    })?;
    Ok(())
}

fn example_dir() -> PathBuf {
    let parameters_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    parameters_path.join("examples").join("network-hhmodel")
}

fn initialize(context: &mut Context) -> Result<(), IxaError> {
    context.init_random(1);

    // Load people from csv and set up some base properties
    loader::init(context);

    // Load parameters from json
    let file_path = example_dir().join("config.json");
    context.load_global_properties(&file_path)?;

    let parameters = context
        .get_global_property_value(Parameters)
        .ok_or_else(|| IxaError::PropertyNotSet {
            name: "Parameters".to_string(),
        })?
        .clone();

    // Load network
    network::init(context, parameters.relative_rate);

    // Initialize incidence report
    incidence_report::init(context)?;

    // Initialize infected person with InfectedBy value equal to their own PersonId
    let to_infect: Vec<PersonId> = vec![context
        .sample_entity(MainRng, Person)
        .expect("network-hhmodel requires a nonempty population")];

    seir::init(context, &to_infect, 1.0);
    Ok(())
}

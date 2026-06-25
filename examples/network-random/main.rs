use ixa::prelude::*;
use ixa::runner::run_with_args;

mod infection;
mod network;
mod parameters;
use std::path::PathBuf;

use parameters::Parameters;

define_entity!(Person);
define_rng!(MainRng);

fn main() {
    run_with_args(|context, _, _| {
        init(context);
        Ok(())
    })
    .unwrap();
}

fn init(context: &mut Context) {
    context.init_random(1);

    // Load parameters from json
    let file_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("examples")
        .join("network-random")
        .join("config.json");
    context.load_global_properties(&file_path).unwrap();

    let parameters = context
        .get_global_property_value(Parameters)
        .unwrap()
        .clone();

    // Load network
    network::init(
        context,
        parameters.population_size,
        parameters.n_connections,
        38421,
    );

    infection::init(context, parameters.n_initial_infected);
}

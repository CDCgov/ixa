use ixa::prelude::*;

pub mod infection;
pub mod network;
pub mod parameters;

use crate::parameters::Parameters;

define_entity!(Person);

pub fn init(context: &mut Context) {
    // Load parameters from json
    parameters::init(context);

    let parameters = context
        .get_global_property_value(Parameters)
        .unwrap()
        .clone();

    // Load network
    network::init(
        context,
        parameters.population_size,
        parameters.connection_p,
        parameters.network_seed,
    );

    infection::init(context, parameters.n_initial_infected);
}

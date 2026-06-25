use ixa::prelude::*;

pub mod infection;
pub mod network;
pub mod parameters;

use crate::parameters::Parameters;

define_entity!(Person);

pub fn init(context: &mut Context) {
    context.init_random(1);

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
        parameters.n_connections,
        38421,
    );

    infection::init(context, parameters.n_initial_infected);
}

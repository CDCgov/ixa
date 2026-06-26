use ixa::prelude::*;

pub mod infection;
pub mod network;
pub mod parameters;

define_entity!(Person);

pub fn init(context: &mut Context) {
    // Load parameters from json
    let parameters = parameters::init(context);

    // Load network
    network::init(
        context,
        parameters.population_size,
        parameters.connection_p,
        parameters.network_seed,
    );

    infection::init(context, parameters.n_initial_infected);
}

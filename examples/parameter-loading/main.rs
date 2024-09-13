use ixa::define_global_property;
use::ixa::define_rng;
use ixa::{context::Context, global_properties::ContextGlobalPropertiesExt};

use serde::{Deserialize, Serialize};

use ixa::random::ContextRandomExt;
mod incidence_report;
mod infection_manager;
mod people;
mod transmission_manager;
mod parameters_loader;

use crate::people::ContextPeopleExt;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ParametersValues {
    population: usize,
    max_time: f64,
    seed: u64,
    foi: f64,
    infection_duration:f64,
    output_dir: String,
    output_file: String,
}

define_global_property!(Parameters, ParametersValues);


fn main() {
    let mut context = Context::new();
    parameters_loader::init(&mut context, "input.yaml");
    
    let parameters = context.get_global_property_value(Parameters).clone();
    context.init_random(parameters.seed);
    
    for _ in 0..parameters.population {
        context.create_person();
    }

    transmission_manager::init(&mut context);
    infection_manager::init(&mut context);
    incidence_report::init(&mut context);

    context.add_plan(parameters.max_time, |context| {
        context.shutdown();
    });
    println!("{:?}", parameters);
    context.execute();
}


// // Load parameters
// fn load_parameters_from_config(ParameterValues, path) {
//     define_global_property!(Parameters, ParameterValues);
// }   
 
// // In the model
// struct ParameterValues {
//     random_seed: u64,
//     population_size: usize,
//     foi: f64,
//     infection_period: u64
// }
// load_parameters_from_config(ParameterValues, "config.yaml");
 
// let params = context.get_global_property(Parameters)
 

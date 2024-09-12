use ixa::define_global_property;
use::ixa::define_rng;
use ixa::{context::Context, global_properties::ContextGlobalPropertiesExt};
use serde::{Deserialize, Serialize};

use ixa::random::ContextRandomExt;

mod incidence_report;
mod infection_manager;
mod people;
mod transmission_manager;

use crate::people::ContextPeopleExt;


#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub struct ParametersValues {
    population: usize,
    max_time: f64,
    seed: u64,
    foi: f64,
    infection_duration:f64,
}

define_rng!(TestRng);
define_global_property!(Parameters, ParametersValues);

fn main() {
    let mut context = Context::new();
    let parameters_values = ParametersValues {
        population: 10,        
        max_time: 20.0,
        seed: 123,
        foi: 0.1,
        infection_duration: 5.0,
    };
    context.set_global_property_value(Parameters, parameters_values);

    
    let parameters = context.get_global_property_value(Parameters);
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
 

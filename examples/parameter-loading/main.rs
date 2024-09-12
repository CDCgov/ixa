use ixa::define_global_property;
use ixa::{context::Context, global_properties::ContextGlobalPropertiesExt};
//use ixa::global_properties::GlobalPropertiesContext;
use serde::{Deserialize, Serialize};

// First, let's assume we already read the parameters in the Parameters struct
#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub struct ParametersValues {
    population: usize,
    max_time: f64,
    seed: u64,
    foi: f64,
    infection_duration:f64,
}

fn main() {
    let mut context = Context::new();
    let parameters = ParametersValues {
        population: 10,        
        max_time: 20.0,
        seed: 123,
        foi: 0.1,
        infection_duration: 5.0,
    };

    define_global_property!(Parameters, ParametersValues);
    
    context.set_global_property_value(Parameters, parameters);

    let parameters = context.get_global_property_value(Parameters);
    
    context.add_plan(parameters.max_time, |context| {
        context.shutdown();
    });
    print!("{:?}", parameters);
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
 

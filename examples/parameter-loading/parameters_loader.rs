use ixa::context::Context;
use ixa::global_properties::ContextGlobalPropertiesExt;

use serde::{Deserialize, Serialize};

use crate::Parameters;
use crate::ParametersValues;

pub fn init(context: &mut Context, file_name: &str) {
    
    let parameters_values = ParametersValues {
        population: 10,        
        max_time: 20.0,
        seed: 123,
        foi: 0.1,
        infection_duration: 5.0,
        output_dir: "examples/parameter-loading".to_string(),
        output_file: "incidence".to_string(),
    };
    context.set_global_property_value(Parameters, parameters_values);
    let parameters = context.get_global_property_value(Parameters).clone();
    println!("Input: {:?}, Values {:?}", file_name, parameters);
}

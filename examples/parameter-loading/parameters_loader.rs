use ixa::context::Context;
use ixa::global_properties::ContextGlobalPropertiesExt;
use std::fmt::Debug;

use ixa::define_global_property;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ParametersValues {
    pub population: usize,
    pub max_time: f64,
    pub seed: u64,
    pub foi: f64,
    pub infection_duration: f64,
    pub output_dir: String,
    pub output_file: String,
}
define_global_property!(Parameters, ParametersValues);

pub fn init_parameters(context: &mut Context, file_name: &str) {
    let parameters_values = context.load_parameters_from_config::<ParametersValues>(file_name);
    context.set_global_property_value(Parameters, parameters_values);
}

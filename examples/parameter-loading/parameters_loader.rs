use ixa::context::Context;
use ixa::global_properties::ContextGlobalPropertiesExt;
use std::fmt::Debug;
use std::fs;

use ixa::define_global_property;

use serde::de::DeserializeOwned;
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

fn load_parameters_from_config<T: 'static + Debug + DeserializeOwned>(
    _context: &mut Context,
    file_name: &str,
) -> T {
    let config_file = fs::read_to_string(file_name).expect("Config file not loaded properly");
    let config: T = toml::from_str(&config_file).expect("could not parse config file as toml");
    config
}

pub fn init_parameters(context: &mut Context, file_name: &str) {
    let parameters_values = load_parameters_from_config::<ParametersValues>(context, file_name);
    context.set_global_property_value(Parameters, parameters_values);
}

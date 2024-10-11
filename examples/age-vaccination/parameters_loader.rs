use ixa::context::Context;
use ixa::global_properties::ContextGlobalPropertiesExt;
use std::fmt::Debug;
use std::path::Path;

use ixa::define_global_property;
use ixa::error::IxaError;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ParametersValues {
    pub birth_rate: f64,
    pub death_rate: f64,
    pub max_time: f64,
    pub seed: u64,
    pub foi: f64,
    pub infection_duration: f64,
    pub output_dir: String,
    pub output_file: String,
    pub output_people_file: String,
    pub people_file: String,
}
define_global_property!(Parameters, ParametersValues);

pub fn init_parameters(context: &mut Context, file_path: &Path) -> Result<(), IxaError> {
    let parameters_json = context.load_parameters_from_json::<ParametersValues>(file_path)?;
    context.set_global_property_value(Parameters, parameters_json);
    Ok(())
}

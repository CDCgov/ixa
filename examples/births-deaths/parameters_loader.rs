use ixa::context::Context;
use ixa::global_properties::ContextGlobalPropertiesExt;
use std::{fmt::Debug, vec};
use std::path::Path;

use ixa::define_global_property;
use ixa::error::IxaError;
use serde::{Deserialize, Serialize};

use crate::population_manager::AgeGroupRisk;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FoiAgeGroups {
    pub group_name: AgeGroupRisk,
    pub foi: f64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ParametersValues {
    pub population: usize,
    pub max_time: f64,
    pub seed: u64,
    pub birth_rate: f64,
    pub foi_groups: Vec<FoiAgeGroups>,
    pub infection_duration: f64,
    pub output_file: String,
    pub demographic_output_file: String,
}
define_global_property!(Parameters, ParametersValues);
//define_global_property!(Foi, HashMap);
pub fn init_parameters(context: &mut Context, file_path: &Path) -> Result<(), IxaError> {
    let parameters_json = context.load_parameters_from_json::<ParametersValues>(file_path)?;
    context.set_global_property_value(Parameters, parameters_json.clone());            
    Ok(())
}


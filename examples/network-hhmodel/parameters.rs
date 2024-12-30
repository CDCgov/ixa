use ixa::context::Context;
use ixa::global_properties::ContextGlobalPropertiesExt;
use std::fmt::Debug;
use std::path::Path;

use ixa::define_global_property;
use ixa::error::IxaError;
use serde::{Deserialize, Serialize};

#[allow(clippy::module_name_repetitions)]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ParametersValues {
    pub incubation_period: f64,
    pub infectious_period: f64,
    pub sar: f64,
    pub shape: f64,
    pub infection_duration: f64,
    pub between_hh_transmission_reduction: f64,
}
define_global_property!(Parameters, ParametersValues);

pub fn init(context: &mut Context, file_name: &str) -> Result<(), IxaError> {
    let current_dir = Path::new(file!()).parent().unwrap();
    let file_path = current_dir.join(file_name);
    let parameters_json = context.load_parameters_from_json::<ParametersValues>(&file_path)?;
    context.set_global_property_value(Parameters, parameters_json)?;
    Ok(())
}

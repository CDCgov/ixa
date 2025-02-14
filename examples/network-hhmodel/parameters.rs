use std::fmt::Debug;

use ixa::define_global_property;
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
    pub output_dir: String,
    pub data_dir: String,
}
define_global_property!(Parameters, ParametersValues);

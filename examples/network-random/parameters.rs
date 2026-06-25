use std::fmt::Debug;

use ixa::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ParametersValues {
    pub infectious_period: f64,
    pub sar: f64,
    pub population_size: usize,
    pub n_connections: usize,
    pub n_initial_infected: usize,
    pub output_dir: String,
    pub data_dir: String,
}
define_global_property!(Parameters, ParametersValues);

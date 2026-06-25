use std::fmt::Debug;

use ixa::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ParametersValues {
    pub generation_interval: f64,
    pub population_size: usize,
    pub n_connections: usize,
    pub n_initial_infected: usize,
    pub output_dir: String,
}
define_global_property!(Parameters, ParametersValues);

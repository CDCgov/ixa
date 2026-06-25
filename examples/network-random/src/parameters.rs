use std::fmt::Debug;

use ixa::prelude::*;
use serde::{Deserialize, Serialize};

use crate::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParametersValues {
    pub generation_interval: f64,
    pub population_size: usize,
    pub n_connections: usize,
    pub n_initial_infected: usize,
}
define_global_property!(Parameters, ParametersValues);

pub fn load(context: &mut Context) {
    let file_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("config.json");
    context.load_global_properties(&file_path).unwrap();
}

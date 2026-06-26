use std::fmt::Debug;
use std::path::PathBuf;

use ixa::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParametersValues {
    pub generation_interval: f64,
    pub population_size: usize,
    pub connection_p: f64,
    pub network_seed: u64,
    pub n_initial_infected: usize,
}
define_global_property!(Parameters, ParametersValues);

pub fn init(context: &mut Context) {
    let file_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("config.json");
    context
        .load_global_properties(&file_path)
        .expect("could not load parameters");
}

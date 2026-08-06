use std::fmt::Debug;
use std::path::Path;

use ixa::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Parameters {
    pub population: usize,
    pub max_time: f64,
    pub seed: u64,
    pub foi: f64,
    pub infection_duration: f64,
    pub output_dir: String,
    pub output_file: String,
}
define_global_property!(ParametersKey, Parameters);

pub trait ParametersExt: PluginContext {
    fn init_parameters(&mut self, file_path: &Path) -> Result<(), IxaError> {
        let parameters_json = self.load_parameters_from_json::<Parameters>(file_path)?;
        self.set_global_property_value(ParametersKey, parameters_json)?;
        Ok(())
    }
    fn get_parameters(&self) -> &Parameters {
        self.get_global_property_value(ParametersKey)
            .as_ref()
            .unwrap()
    }
}

impl ParametersExt for Context {}

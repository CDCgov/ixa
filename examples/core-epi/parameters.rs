//! Model parameters, stored as a global property.
use ixa::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Parameters {
    pub seed: u64,
    pub population: usize,
    pub initial_infections: usize,
    /// Infection attempts per day while infectious; with homogeneous mixing
    /// R0 is `intrinsic_infectiousness * infectious_period`.
    pub intrinsic_infectiousness: f64,
    /// Mean infectious period in days.
    pub infectious_period: f64,
}

impl Default for Parameters {
    fn default() -> Self {
        Self {
            seed: 42,
            population: 1000,
            initial_infections: 5,
            intrinsic_infectiousness: 0.4,
            infectious_period: 5.0,
        }
    }
}

define_global_property!(ParametersKey, Parameters);

pub trait ParametersExt: PluginContext {
    fn set_parameters(&mut self, parameters: Parameters) {
        self.set_global_property_value(ParametersKey, parameters)
            .unwrap();
    }

    fn get_parameters(&self) -> &Parameters {
        self.get_global_property_value(ParametersKey)
            .as_ref()
            .expect("parameters not set")
    }
}

impl ParametersExt for Context {}

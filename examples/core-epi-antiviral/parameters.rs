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
    /// Share of infected people who get the antiviral.
    pub antiviral_coverage: f64,
    /// Days from infection to the start of treatment.
    pub antiviral_delay: f64,
    /// Factor on infectiousness while treated.
    pub antiviral_effect: f64,
}

impl Default for Parameters {
    fn default() -> Self {
        Self {
            seed: 42,
            population: 1000,
            initial_infections: 5,
            intrinsic_infectiousness: 0.5,
            infectious_period: 5.0,
            antiviral_coverage: 0.6,
            antiviral_delay: 1.0,
            antiviral_effect: 0.25,
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

//! Model parameters, stored as a global property.
use ixa::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Parameters {
    pub seed: u64,
    pub population: usize,
    pub initial_infections: usize,
    /// Intrinsic infectiousness: infection attempts per day, before the
    /// setting's `(n - 1)^alpha` multiplier.
    pub intrinsic_infectiousness: f64,
    /// Density exponent for the household: 1 makes attempts scale with the
    /// number of other members, 0 makes them independent of household size.
    pub household_alpha: f64,
    /// Density exponent for the community; 0 makes the multiplier 1.
    pub community_alpha: f64,
    /// Mean infectious period in days.
    pub infectious_period: f64,
    /// Probability that a transmission attempt finds the person at home rather
    /// than in the community.
    pub household_probability: f64,
    /// Household sizes and their probabilities.
    pub household_sizes: Vec<(usize, f64)>,
}

impl Default for Parameters {
    fn default() -> Self {
        Self {
            seed: 42,
            population: 1000,
            initial_infections: 5,
            intrinsic_infectiousness: 0.35,
            household_alpha: 1.0,
            community_alpha: 0.0,
            infectious_period: 5.0,
            household_probability: 0.4,
            household_sizes: vec![(1, 0.3), (2, 0.35), (3, 0.15), (4, 0.12), (5, 0.08)],
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

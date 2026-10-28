//! Model parameters, stored as a global property.
use ixa::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Parameters {
    pub seed: u64,
    /// Line list of people, one row per person: `age,household_id,school_id,
    /// workplace_id` (the last two may be empty). Relative to the crate root.
    pub population_file: String,
    pub initial_infections: usize,
    /// Intrinsic infectiousness: infection attempts per day, before the
    /// setting's `(n - 1)^alpha` multiplier.
    pub intrinsic_infectiousness: f64,
    /// Mean infectious period in days.
    pub infectious_period: f64,
    /// Density exponent per setting: 1 makes attempts scale with the number
    /// of other members, 0 makes them independent of size.
    pub household_alpha: f64,
    pub school_alpha: f64,
    pub workplace_alpha: f64,
    pub community_alpha: f64,
    /// Share of time per setting, renormalized per person over the settings
    /// they belong to.
    pub household_ratio: f64,
    pub school_ratio: f64,
    pub workplace_ratio: f64,
    pub community_ratio: f64,
}

impl Default for Parameters {
    fn default() -> Self {
        Self {
            seed: 42,
            population_file: "examples/core-epi-settings/input/people.csv".to_string(),
            initial_infections: 5,
            intrinsic_infectiousness: 0.2,
            infectious_period: 5.0,
            household_alpha: 1.0,
            school_alpha: 0.5,
            workplace_alpha: 0.5,
            community_alpha: 0.0,
            household_ratio: 0.4,
            school_ratio: 0.3,
            workplace_ratio: 0.3,
            community_ratio: 0.3,
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

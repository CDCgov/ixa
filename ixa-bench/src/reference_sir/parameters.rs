use derive_builder::Builder;
use serde::{Deserialize, Serialize};

/// Relative weights for the settings an infectious person can contact a
/// susceptible from. The probability of drawing the next contact from a
/// given setting is `setting_weight / sum_of_weights`.
#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
pub struct Itinerary {
    pub household: f64,
    pub community: f64,
}

impl Default for Itinerary {
    fn default() -> Self {
        Self {
            household: 0.5,
            community: 0.5,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Builder)]
pub struct Parameters {
    #[builder(default = "1.5")]
    pub r0: f64,

    #[builder(default = "3.0")]
    pub infectious_period: f64,

    #[builder(default = "1000")]
    pub population: usize,

    #[builder(default = "5")]
    pub initial_infections: usize,

    #[builder(default = "0")]
    pub seed: u64,

    #[builder(default = "100.0")]
    pub max_time: f64,

    #[builder(default)]
    pub itinerary: Itinerary,
}

impl Default for Parameters {
    fn default() -> Self {
        ParametersBuilder::default().build().unwrap()
    }
}

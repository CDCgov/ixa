use derive_builder::Builder;
use serde::{Deserialize, Serialize};

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
}

impl Default for Parameters {
    fn default() -> Self {
        ParametersBuilder::default().build().unwrap()
    }
}

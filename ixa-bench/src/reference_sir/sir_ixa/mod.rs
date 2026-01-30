use ixa::{define_data_plugin, define_global_property, define_rng};
use serde::{Deserialize, Serialize};

use crate::reference_sir::{ModelStats, Parameters};

pub mod entities;
pub mod legacy;

#[derive(Clone, Copy, Debug, Serialize, Deserialize, Default)]
pub struct ModelOptions {}

define_global_property!(Params, Parameters);
define_global_property!(Options, ModelOptions);

define_rng!(NextPersonRng);
define_rng!(NextEventRng);

define_data_plugin!(ModelStatsPlugin, ModelStats, |context| {
    let params = context.get_global_property_value(Params).unwrap();
    ModelStats::new(params.initial_infections, params.population, 0.2)
});

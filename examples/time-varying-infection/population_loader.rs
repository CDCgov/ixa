use ixa::prelude::*;
use serde::{Deserialize, Serialize};

use crate::parameters_loader::Parameters;

#[derive(Deserialize, Serialize, Copy, Clone, PartialEq, Eq, Debug, Hash)]
pub enum DiseaseStatusValue {
    S,
    I,
    R,
}

define_person_property_with_default!(DiseaseStatus, DiseaseStatusValue, DiseaseStatusValue::S);
define_person_property_with_default!(InfectionTime, Option<f64>, None);

pub fn init(context: &mut Context) {
    let parameters = context
        .get_global_property_value(Parameters)
        .unwrap()
        .clone();
    for _ in 0..parameters.population {
        context.add_person(()).unwrap();
    }
}

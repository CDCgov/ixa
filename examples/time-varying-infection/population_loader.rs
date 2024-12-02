use ixa::context::Context;
use ixa::global_properties::ContextGlobalPropertiesExt;
use ixa::people::ContextPeopleExt;
use ixa::{define_person_property, define_person_property_with_default};
use ordered_float::OrderedFloat;
use serde::{Deserialize, Serialize};
use strum_macros::EnumIter;

use crate::parameters_loader::Parameters;

#[derive(Deserialize, Serialize, Copy, Clone, PartialEq, Eq, Debug, EnumIter, Hash)]
pub enum DiseaseStatus {
    S,
    I,
    R,
}

define_person_property_with_default!(DiseaseStatusType, DiseaseStatus, DiseaseStatus::S);
define_person_property_with_default!(InfectionTime, Option<OrderedFloat<f64>>, None);

pub fn init(context: &mut Context) {
    let parameters = context
        .get_global_property_value(Parameters)
        .unwrap()
        .clone();
    for _ in 0..parameters.population {
        context.add_person(()).unwrap();
    }
}

use ixa::context::Context;
use ixa::people::ContextPeopleExt;
use ixa::{define_person_property, define_person_property_with_default};
use serde::{Deserialize, Serialize};
use strum_macros::EnumIter;

use crate::POPULATION;

#[derive(Deserialize, Serialize, Copy, Clone, PartialEq, Eq, Debug, EnumIter, Hash)]
pub enum InfectionStatus {
    S,
    I,
    R,
}

define_person_property_with_default!(InfectionStatusType, InfectionStatus, InfectionStatus::S);

pub fn init(context: &mut Context) {
    for _ in 0..POPULATION {
        context.add_person(()).unwrap();
    }
}

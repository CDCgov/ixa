use ixa::prelude::*;
use ixa::trace;
use serde::{Deserialize, Serialize};

use crate::POPULATION;

// ANCHOR: InfectionStatusValue
#[derive(Debug, Hash, Eq, PartialEq, Clone, Copy, Serialize, Deserialize)]
pub enum InfectionStatusValue {
    S,
    I,
    R,
}
// ANCHOR_END: InfectionStatusValue

// In this model, people only have a single property, their infection status.
// ANCHOR: define_person_property
define_person_property_with_default!(
    InfectionStatus,
    InfectionStatusValue,
    InfectionStatusValue::S
);
// ANCHOR_END: define_person_property

// Populates the "world" with the `POPULATION` number of people.
pub fn init(context: &mut Context) {
    trace!("Initializing people");
    for _ in 0..POPULATION {
        context.add_person(()).unwrap();
    }
}

use ixa::context::Context;
use ixa::trace;
use ixa::{define_person_property_with_default, ContextPeopleExt};

use serde::{Deserialize, Serialize};

use crate::POPULATION;

#[derive(Debug, Hash, Eq, PartialEq, Clone, Copy, Serialize, Deserialize)]
pub enum InfectionStatusValue {
    S,
    I,
    R,
}

// In this model, people only have a single property, their infection status.
define_person_property_with_default!(
    InfectionStatus,
    InfectionStatusValue,
    InfectionStatusValue::S
);

/// Populates the "world" with the `POPULATION` number of people.
pub fn init(context: &mut Context) {
    trace!("Initializing people");
    for _ in 0..POPULATION {
        context.add_person(()).unwrap();
    }
}

use crate::POPULATION;
use ixa::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Hash, Eq, PartialEq, Clone, Copy, Serialize, Deserialize)]
pub enum InfectionStatusValue {
    S,
    I,
    R,
}

define_person_property_with_default!(
    InfectionStatus,         // Property Name
    InfectionStatusValue,    // Type of the Property Values
    InfectionStatusValue::S  // Default value used when a person is added to the simulation
);

/// Populates the "world" with the `POPULATION` number of people.
pub fn init(context: &mut Context) {
    trace!("Initializing people");
    for _ in 0..POPULATION {
        context.add_person(()).expect("failed to add person");
    }
}

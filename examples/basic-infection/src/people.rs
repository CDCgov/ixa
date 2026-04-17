use ixa::prelude::*;
use ixa::trace;
use serde::Serialize;

use crate::POPULATION;

define_entity!(Person);

// In this model, people only have a single property, their infection status.
#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy, Serialize)]
pub enum InfectionStatus {
    S,
    I,
    R,
}

impl_property!(InfectionStatus, Person, default_const = InfectionStatus::S);

/// Populates the "world" with the `POPULATION` number of people.
pub fn init(context: &mut Context) {
    trace!("Initializing people");
    for _ in 0..POPULATION {
        let _ = context.add_entity(Person).unwrap();
    }
}

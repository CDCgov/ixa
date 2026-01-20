use ixa::prelude::*;
use ixa::trace;

use crate::POPULATION;

define_entity!(Person);

// In this model, people only have a single property, their infection status.
define_property!(
    enum InfectionStatus {
        S,
        I,
        R,
    },
    Person,
    default_const = InfectionStatus::S
);

/// Populates the "world" with the `POPULATION` number of people.
pub fn init(context: &mut Context) {
    trace!("Initializing people");
    for _ in 0..POPULATION {
        context.add_entity::<Person, _>(()).unwrap();
    }
}

use ixa::prelude::*;
use ixa::trace;

use crate::POPULATION;

// ANCHOR: define_property
define_entity!(Person);
define_property!(
    // The type of the property
    enum InfectionStatus {
        S,
        I,
        R,
    },
    // The entity the property is associated with
    Person,
    // The property's default value for newly created `Person` entities
    default_const = InfectionStatus::S
);
// ANCHOR_END: define_property

// ANCHOR: init
/// Populates the "world" with the `POPULATION` number of people.
pub fn init(context: &mut Context) {
    trace!("Initializing people");
    for _ in 0..POPULATION {
        let _ = context.add_entity(q!(Person)).expect("failed to add person");
    }
}
// ANCHOR_END: init

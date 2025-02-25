use ixa::{define_person_property_with_default, trace, Context, ContextPeopleExt};
use serde::{Deserialize, Serialize};
use crate::POPULATION;

#[derive(Debug, Hash, Eq, PartialEq, Clone, Copy, Serialize, Deserialize)]
pub enum InfectionStatusValue {
  S,
  I,
  R,
}

define_person_property_with_default!(
    InfectionStatus,        // Property Name
    InfectionStatusValue,   // Type of the Property Values
    InfectionStatusValue::S // The default value assigned to each person
);

/// Populates the "world" with the `POPULATION` number of people.
pub fn init(context: &mut Context) {
  trace!("Initializing people");
  for _ in 0..POPULATION {
    context.add_person(()).expect("failed to add person");
  }
}

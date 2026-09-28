use std::path::Path;

use ixa::prelude::*;

use crate::people::{Age, InfectionStatus, Person, Vaccinated};

pub fn generate(path: &Path) -> Result<(), IxaError> {
    let mut context = Context::new();
    context.add_entity(with!(Person, Age(8)))?;
    context.add_entity(with!(
        Person,
        Age(34),
        InfectionStatus::Infected,
        Vaccinated(true)
    ))?;
    context.add_entity(with!(Person, Age(67), Vaccinated(true)))?;
    context.save_population(path)
}

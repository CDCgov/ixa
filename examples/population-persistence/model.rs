use std::path::Path;

use ixa::prelude::*;

use crate::people::{Age, InfectionStatus, Person};

#[derive(Debug, PartialEq, Eq)]
pub struct PopulationSummary {
    pub people: usize,
    pub infected: usize,
    pub total_age: usize,
}

pub fn run(path: &Path) -> Result<PopulationSummary, IxaError> {
    let mut context = Context::from_population(path)?;
    context.add_plan(1.0, |context| {
        for person in context.get_entity_iterator::<Person>() {
            if context.get_property::<Person, InfectionStatus>(person) == InfectionStatus::Infected
            {
                context.set_property(person, InfectionStatus::Recovered);
            }
        }
    });

    let people = context.get_entity_count::<Person>();
    let infected = context.query_entity_count(with!(Person, InfectionStatus::Infected));
    let total_age = context
        .get_entity_iterator::<Person>()
        .map(|person| usize::from(context.get_property::<Person, Age>(person).0))
        .sum();

    context.execute();
    Ok(PopulationSummary {
        people,
        infected,
        total_age,
    })
}

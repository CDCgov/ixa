use ixa::prelude::*;
use serde::Deserialize;

define_property!(HouseholdId, u32);
define_entity_with_properties!(Household { HouseholdId });

define_property!(Age, u8);

define_property!(IsAlive, bool);

define_property!(
    enum InfectionStatus {
        Susceptible,
        Infected,
        Recovered,
    }
);

define_entity_with_properties!(
    Person {
        Age,
        IsAlive = true,
        Property<InfectionStatus> = InfectionStatus::Susceptible,
    }
);

#[derive(Deserialize)]
struct PersonRecord {
    age: u8,
    household: u32,
}

fn main() -> anyhow::Result<()> {
    let mut context = Context::new();

    let data = std::fs::read_to_string("src/people.json")?;
    let people: Vec<PersonRecord> = serde_json::from_str(&data)?;

    for record in &people {
        let init = Person::build()
            .age(record.age)
            .build()
            .map_err(anyhow::Error::msg)?;
        context.add_entity(init).map_err(anyhow::Error::msg)?;
    }

    println!("Loaded {} people", people.len());

    Ok(())
}

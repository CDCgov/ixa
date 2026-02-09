mod entities;
mod population_loader;

use entities::*;
use ixa::prelude::*;
use population_loader::{load_synthetic_population, SyntheticPopRecord};

fn main() -> anyhow::Result<()> {
    let mut context = Context::new();

    let people: Vec<SyntheticPopRecord> = load_synthetic_population("people.json")?;

    for record in &people {
        let person = Person::new()
            .age(record.age)
            .household_tag(record.household)
            .build()?;
        context.add_entity(person)?;
    }

    println!("Loaded {} people", people.len());

    Ok(())
}

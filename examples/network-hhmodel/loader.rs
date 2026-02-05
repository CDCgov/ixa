use std::fs::File;

use csv::Reader;
use ixa::impl_property;
use ixa::prelude::*;
use serde::{Deserialize, Serialize};

use crate::{example_dir, Person, PersonId};

#[derive(Serialize, Deserialize, Copy, Clone, PartialEq, Eq, Debug, Hash)]
pub struct Id(pub u16);
impl_property!(Id, Person);

#[derive(Serialize, Deserialize, Copy, Clone, PartialEq, Eq, Debug, Hash)]
pub enum AgeGroup {
    AgeUnder5,
    Age5to17,
    Age18to64,
    Age65Plus,
}
impl_property!(AgeGroup, Person);

#[derive(Serialize, Deserialize, Copy, Clone, PartialEq, Eq, Debug, Hash)]
pub enum Sex {
    Female,
    Male,
}
impl_property!(Sex, Person);

#[derive(Serialize, Deserialize, Copy, Clone, PartialEq, Eq, Debug, Hash)]
pub struct HouseholdId(pub u16);
impl_property!(HouseholdId, Person);

#[derive(Deserialize, Debug)]
struct PeopleRecord {
    id: Id,
    age_group: AgeGroup,
    sex: Sex,
    household_id: HouseholdId,
}

fn create_person_from_record(context: &mut Context, record: &PeopleRecord) -> PersonId {
    context
        .add_entity((record.id, record.age_group, record.sex, record.household_id))
        .unwrap()
}

pub fn open_csv(file_name: &str) -> Reader<File> {
    let current_dir = example_dir();
    let file_path = current_dir.join(file_name);
    csv::Reader::from_path(file_path).unwrap()
}

pub fn init(context: &mut Context) -> Vec<PersonId> {
    // Load csv and deserialize records
    let mut reader = open_csv("Households.csv");
    let mut people = Vec::new();

    for result in reader.deserialize() {
        let record: PeopleRecord = result.expect("Failed to parse record");
        people.push(create_person_from_record(context, &record));
    }

    context.index_property::<Person, Id>();
    context.index_property::<Person, HouseholdId>();

    people
}

#[cfg(test)]
mod tests {
    use ixa::context::Context;
    use ixa::random::ContextRandomExt;

    use super::*;

    const EXPECTED_ROWS: usize = 1606;

    #[test]
    fn test_init_expected_rows() {
        let mut context = Context::new();
        context.init_random(42);
        init(&mut context);
        assert_eq!(context.get_entity_count::<Person>(), EXPECTED_ROWS);
    }

    #[test]
    fn test_some_people_load_correctly() {
        let mut context = Context::new();
        context.init_random(42);

        let people = init(&mut context);

        let person = people[0];
        assert!(context.match_entity(
            person,
            (Id(676), AgeGroup::Age18to64, Sex::Female, HouseholdId(1))
        ));

        let person = people[246];
        assert!(context.match_entity(
            person,
            (Id(213), AgeGroup::AgeUnder5, Sex::Female, HouseholdId(162))
        ));

        let person = people[1591];
        assert!(context.match_entity(
            person,
            (Id(1591), AgeGroup::Age65Plus, Sex::Male, HouseholdId(496))
        ));
    }
}

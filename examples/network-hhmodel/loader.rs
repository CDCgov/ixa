use crate::example_dir;
use csv::Reader;
use ixa::prelude::*;
use ixa::PersonId;
use serde::Deserialize;
use serde_derive::Serialize;
use std::fs::File;

define_person_property!(Id, u16);

#[derive(Serialize, Deserialize, Copy, Clone, PartialEq, Eq, Debug)]
pub enum AgeGroupValue {
    AgeUnder5,
    Age5to17,
    Age18to64,
    Age65Plus,
}
define_person_property!(AgeGroup, AgeGroupValue);

#[derive(Serialize, Deserialize, Copy, Clone, PartialEq, Eq, Debug, Hash)]
pub enum SexValue {
    Female,
    Male,
}
define_person_property!(Sex, SexValue);

define_person_property!(HouseholdId, u16);

#[derive(Deserialize, Debug)]
struct PeopleRecord {
    id: u16,
    age_group: AgeGroupValue,
    sex: SexValue,
    household_id: u16,
}

fn create_person_from_record(context: &mut Context, record: &PeopleRecord) -> PersonId {
    context
        .add_person((
            (Id, record.id),
            (AgeGroup, record.age_group),
            (Sex, record.sex),
            (HouseholdId, record.household_id),
        ))
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

    context.index_property(Id);
    context.index_property(HouseholdId);

    people
}

#[cfg(test)]
mod tests {
    use super::*;
    use ixa::{context::Context, random::ContextRandomExt};

    const EXPECTED_ROWS: usize = 1606;

    #[test]
    fn test_init_expected_rows() {
        let mut context = Context::new();
        context.init_random(42);
        init(&mut context);
        assert_eq!(context.get_current_population(), EXPECTED_ROWS);
    }

    #[test]
    fn test_some_people_load_correctly() {
        let mut context = Context::new();
        context.init_random(42);

        let people = init(&mut context);

        let person = people[0];
        assert!(context.match_person(person, (Id, 676)));
        assert!(context.match_person(person, (AgeGroup, AgeGroupValue::Age18to64)));
        assert!(context.match_person(person, (Sex, SexValue::Female)));
        assert!(context.match_person(person, (HouseholdId, 1)));

        let person = people[246];
        assert!(context.match_person(person, (Id, 213)));
        assert!(context.match_person(person, (AgeGroup, AgeGroupValue::AgeUnder5)));
        assert!(context.match_person(person, (Sex, SexValue::Female)));
        assert!(context.match_person(person, (HouseholdId, 162)));

        let person = people[1591];
        assert!(context.match_person(person, (Id, 1591)));
        assert!(context.match_person(person, (AgeGroup, AgeGroupValue::Age65Plus)));
        assert!(context.match_person(person, (Sex, SexValue::Male)));
        assert!(context.match_person(person, (HouseholdId, 496)));
    }
}

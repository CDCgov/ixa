use std::fs::File;

use csv::Reader;
use ixa::impl_property;
use ixa::prelude::*;
use serde::{Deserialize, Serialize};

use crate::{example_dir, Person};

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

pub fn open_csv(file_name: &str) -> Reader<File> {
    let current_dir = example_dir();
    let file_path = current_dir.join(file_name);
    csv::Reader::from_path(file_path).unwrap()
}

pub fn init(context: &mut Context) {
    // Load csv and deserialize records
    let mut reader = open_csv("Households.csv");

    for result in reader.deserialize() {
        let record: PeopleRecord = result.expect("Failed to parse record");
        context
            .add_entity(with!(
                Person,
                record.id,
                record.age_group,
                record.sex,
                record.household_id
            ))
            .unwrap();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXPECTED_ROWS: usize = 1606;

    #[test]
    fn test_init_expected_rows() {
        let mut context = Context::new();
        init(&mut context);
        assert_eq!(context.get_entity_count::<Person>(), EXPECTED_ROWS);
    }

    // Check there is exactly one matching entity
    fn assert_exists1(
        context: &Context,
        id: Id,
        age_group: AgeGroup,
        sex: Sex,
        hh_id: HouseholdId,
    ) {
        assert_eq!(
            context.query_entity_count(with!(Person, id, age_group, sex, hh_id)),
            1
        );
    }

    #[test]
    fn test_some_people_load_correctly() {
        let mut context = Context::new();
        init(&mut context);

        // e.g., the person with data id 676 should be 18-64, female, in household 1
        assert_exists1(
            &context,
            Id(676),
            AgeGroup::Age18to64,
            Sex::Female,
            HouseholdId(1),
        );

        assert_exists1(
            &context,
            Id(213),
            AgeGroup::AgeUnder5,
            Sex::Female,
            HouseholdId(162),
        );

        assert_exists1(
            &context,
            Id(1591),
            AgeGroup::Age65Plus,
            Sex::Male,
            HouseholdId(496),
        );
    }
}

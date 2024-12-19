use std::path::Path;

use ixa::context::Context;
use ixa::define_person_property;
use ixa::{ContextPeopleExt, PersonId,};
use serde::Deserialize;
use csv::Reader;
use std::fs::File;
use crate::network_loader::load_network;
use crate::parameter_loader::load_parameters;

#[derive(Deserialize, Copy, Clone, PartialEq, Eq, Debug, Hash)]
pub enum AgeGroupValue {
    U5,
    U18,
    Adult,
    Old,
}

#[derive(Deserialize, Copy, Clone, PartialEq, Eq, Debug, Hash)]
pub enum SexValue {
    Female,
    Male,
}

#[derive(Deserialize, Debug)]
struct PeopleRecord {
    id: u16,
    age_group: AgeGroupValue,
    sex: SexValue,
    household_id: u16,
}

define_person_property!(Id, u16);
define_person_property!(AgeGroup, AgeGroupValue);
define_person_property!(Sex, SexValue);
define_person_property!(HouseholdId, u16);

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
    let current_dir = Path::new(file!()).parent().unwrap();
    let file_path = current_dir.join(file_name);
    csv::Reader::from_path(file_path).unwrap()
}

pub fn init(context: &mut Context) {
    // Load csv and deserialize records
    let mut reader = open_csv("synthetic_households_us.csv");
    let mut people = Vec::new();

    for result in reader.deserialize() {
        let record: PeopleRecord = result.expect("Failed to parse record");
        people.push(create_person_from_record(context, &record));
    }

    context.index_property(Id);
    context.index_property(HouseholdId);

    load_parameters(context);
    load_network(context, &people);

}

#[cfg(test)]
mod tests {

    use super::*;
    use ixa::{
        context::Context,
        random::ContextRandomExt,
    };

    const EXPECTED_ROWS: usize = 12258;

    #[test]
    fn test_init_expected_rows() {
        let mut context = Context::new();
        context.init_random(42);
        init(&mut context);
        assert_eq!(context.get_current_population(), EXPECTED_ROWS);
    }

}

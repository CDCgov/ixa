use std::collections::HashSet;
use std::path::Path;

use ixa::context::Context;
use ixa::define_person_property;
use ixa::define_edge_type;
use ixa::{ContextPeopleExt, PersonId, ContextNetworkExt};
use serde::Deserialize;

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

#[derive(Deserialize, Copy, Clone, PartialEq, Eq, Debug, Hash)]
pub enum EdgeKind {
    HH,
    U5,
    U18,
}

impl Default for EdgeKind {
    fn default() -> Self {
        EdgeKind::HH
    }
}

define_edge_type!(EdgeKindType, EdgeKind);

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

fn create_household_networks(context: &mut Context, people: &Vec<PersonId>) {
    let mut households = HashSet::new();
    for person_id in people.iter() {
        let household_id = context
                                .get_person_property(*person_id, HouseholdId);
        if !households.contains(&household_id) {
            households.insert(household_id);

            let mut members = context.query_people((HouseholdId, household_id));
            // create a dense network 
            while members.len() > 0 {
                let person = members.pop().unwrap();
                for other_person in members.iter() {
                    context.add_edge_bidi::<EdgeKindType>(person, *other_person, 1.0, EdgeKind::HH).unwrap();
                }
            }
        }
    }
}

pub fn init(context: &mut Context) {
    // Load csv and deserialize records
    let current_dir = Path::new(file!()).parent().unwrap();
    let mut reader = csv::Reader::from_path(current_dir.join("synthetic_households_us.csv")).unwrap();
    let mut people = Vec::new();

    for result in reader.deserialize() {
        let record: PeopleRecord = result.expect("Failed to parse record");
        people.push(create_person_from_record(context, &record));
    }

    context.index_property(Id);
    context.index_property(HouseholdId);

    // Create dense household networks
    create_household_networks(context, &people);
}

#[cfg(test)]
mod tests {

    use super::*;
    use ixa::{
        context::Context,
        random::ContextRandomExt,
    };

    const EXPECTED_ROWS: usize = 12258;
    const N_SIZE_12: usize = 1;
    const N_SIZE_11: usize = 3;
    const N_SIZE_3:  usize = 987;

    #[test]
    fn test_init_expected_rows() {
        let mut context = Context::new();
        context.init_random(42);
        init(&mut context);
        assert_eq!(context.get_current_population(), EXPECTED_ROWS);
    }

    #[test]
    fn test_expected_12_member_household() {
        let mut context = Context::new();
        context.init_random(42);
        init(&mut context);
        let deg11 = context.find_people_by_degree::<EdgeKindType>(11);
        assert_eq!(deg11.len(), 12 * N_SIZE_12);
    }

    #[test]
    fn test_expected_11_member_household() {
        let mut context = Context::new();
        context.init_random(42);
        init(&mut context);
        let deg10 = context.find_people_by_degree::<EdgeKindType>(10);
        assert_eq!(deg10.len(), 11 * N_SIZE_11);
    }

    #[test]
    fn test_expected_3_member_household() {
        let mut context = Context::new();
        context.init_random(42);
        init(&mut context);
        let deg10 = context.find_people_by_degree::<EdgeKindType>(2);
        assert_eq!(deg10.len(), 3 * N_SIZE_3);
    }

}

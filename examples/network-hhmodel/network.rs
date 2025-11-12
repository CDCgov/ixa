use ixa::prelude::*;
use ixa::{EdgeType, HashSet, HashSetExt, PersonId};
use serde::Deserialize;

use crate::loader::{open_csv, HouseholdId, Id};

define_edge_type!(Household, ());
define_edge_type!(AgeUnder5, ());
define_edge_type!(Age5to17, ());

#[derive(Deserialize, Debug)]
struct EdgeRecord {
    v1: u16,
    v2: u16,
}

fn create_household_networks(context: &mut Context, people: &[PersonId]) {
    let mut households = HashSet::new();
    for person_id in people {
        let household_id = context.get_person_property(*person_id, HouseholdId);
        if households.insert(household_id) {
            let mut members: Vec<PersonId> = Vec::new();
            context.with_query_people_results((HouseholdId, household_id), &mut |results| {
                members = results.to_owned_vec()
            });
            // create a dense network
            while let Some(person) = members.pop() {
                for other_person in &members {
                    context
                        .add_edge_bidi::<Household>(person, *other_person, 1.0, ())
                        .unwrap();
                }
            }
        }
    }
}

fn load_edge_list<T: EdgeType + 'static>(context: &mut Context, file_name: &str, value: T::Value) {
    let mut reader = open_csv(file_name);

    for result in reader.deserialize() {
        let record: EdgeRecord = result.expect("Failed to parse edge");
        let mut p1_vec = Vec::new();
        context.with_query_people_results((Id, record.v1), &mut |people| {
            p1_vec = people.to_owned_vec()
        });
        assert_eq!(p1_vec.len(), 1);
        let p1 = p1_vec[0];
        let mut p2_vec = Vec::new();
        context.with_query_people_results((Id, record.v2), &mut |people| {
            p2_vec = people.to_owned_vec()
        });
        assert_eq!(p2_vec.len(), 1);
        let p2 = p2_vec[0];
        context.add_edge_bidi::<T>(p1, p2, 1.0, value).unwrap();
    }
}

pub fn init(context: &mut Context, people: &[PersonId]) {
    // Create dense household networks
    create_household_networks(context, people);

    // Add U5 edges from csv
    load_edge_list::<AgeUnder5>(context, "AgeUnder5Edges.csv", ());

    // Add U18 edges from csv
    load_edge_list::<Age5to17>(context, "Age5to17Edges.csv", ());
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::{loader, network};

    const N_SIZE_12: usize = 1;
    const N_SIZE_11: usize = 1;
    const N_SIZE_3: usize = 122;

    #[test]
    fn test_expected_12_member_household() {
        let mut context = Context::new();
        context.init_random(42);
        let people = loader::init(&mut context);
        network::init(&mut context, &people);
        let deg11 = context.find_people_by_degree::<Household>(11);
        assert_eq!(deg11.len(), 12 * N_SIZE_12);
    }

    #[test]
    fn test_expected_11_member_household() {
        let mut context = Context::new();
        context.init_random(42);
        let people = loader::init(&mut context);
        network::init(&mut context, &people);
        let deg10 = context.find_people_by_degree::<Household>(10);
        assert_eq!(deg10.len(), 11 * N_SIZE_11);
    }

    #[test]
    fn test_expected_3_member_household() {
        let mut context = Context::new();
        context.init_random(42);
        let people = loader::init(&mut context);
        network::init(&mut context, &people);
        let deg10 = context.find_people_by_degree::<Household>(2);
        assert_eq!(deg10.len(), 3 * N_SIZE_3);
    }
}

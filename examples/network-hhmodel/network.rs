use ixa::network::edge::EdgeType;
use ixa::prelude::*;
use ixa::{HashSet, HashSetExt};
use rand_distr::Bernoulli;
use serde::Deserialize;

use crate::loader::{open_csv, HouseholdId, Id};
use crate::parameters::Parameters;
use crate::{Person, PersonId};

define_edge_type!(struct Household, Person);
define_edge_type!(struct AgeUnder5, Person);
define_edge_type!(struct Age5to17, Person);

define_rng!(NetworkRng);

#[derive(Deserialize, Debug)]
struct EdgeRecord {
    v1: u16,
    v2: u16,
}

fn create_household_networks(context: &mut Context, people: &[PersonId]) {
    let mut households = HashSet::new();
    for person_id in people {
        let household_id: HouseholdId = context.get_property(*person_id);
        if households.insert(household_id) {
            let mut members: Vec<PersonId> = Vec::new();
            context.with_query_results(with!(Person, household_id), &mut |results| {
                members = results.to_owned_vec()
            });
            // create a dense network
            while let Some(person) = members.pop() {
                for other_person in &members {
                    context
                        .add_edge_bidi(person, *other_person, 1.0, Household)
                        .unwrap();
                }
            }
        }
    }
}

fn load_edge_list<ET: EdgeType<Person>>(context: &mut Context, file_name: &str, inner: ET) {
    let mut reader = open_csv(file_name);

    for result in reader.deserialize() {
        let record: EdgeRecord = result.expect("Failed to parse edge");
        let mut p1_vec = Vec::new();
        context.with_query_results(with!(Person, Id(record.v1)), &mut |people| {
            p1_vec = people.to_owned_vec()
        });
        assert_eq!(p1_vec.len(), 1);
        let p1 = p1_vec[0];
        let mut p2_vec = Vec::new();
        context.with_query_results(with!(Person, Id(record.v2)), &mut |people| {
            p2_vec = people.to_owned_vec()
        });
        assert_eq!(p2_vec.len(), 1);
        let p2 = p2_vec[0];
        context.add_edge_bidi(p1, p2, 1.0, inner.clone()).unwrap();
    }
}

fn sar_to_beta(sar: f64, infectious_period: f64) -> f64 {
    1.0 - (1.0 - sar).powf(1.0 / infectious_period)
}

/// Get all the contacts a person will have over a certain duration
pub fn get_contacts(context: &Context, person_id: PersonId, duration: f64) -> HashSet<PersonId> {
    let parameters = context
        .get_global_property_value(Parameters)
        .unwrap()
        .clone();

    // Probability of contact during the duration. Note that this assumes that the duration is not too high!
    let p_hh = duration * sar_to_beta(parameters.sar, parameters.incubation_period);
    let p_other = duration
        * sar_to_beta(
            parameters.sar / parameters.between_hh_transmission_reduction,
            parameters.incubation_period,
        );

    let mut infectees = HashSet::new();
    infectees.extend(
        get_contacts_by_edge_type::<Household>(context, person_id)
            .iter()
            .filter(|_| context.sample_distr(NetworkRng, Bernoulli::new(p_hh).unwrap())),
    );

    infectees.extend(
        get_contacts_by_edge_type::<AgeUnder5>(context, person_id)
            .iter()
            .filter(|_| context.sample_distr(NetworkRng, Bernoulli::new(p_other).unwrap())),
    );

    infectees.extend(
        get_contacts_by_edge_type::<Age5to17>(context, person_id)
            .iter()
            .filter(|_| context.sample_distr(NetworkRng, Bernoulli::new(p_other).unwrap())),
    );

    infectees
}

/// Get the contacts by edge type
fn get_contacts_by_edge_type<ET: EdgeType<Person> + 'static>(
    context: &Context,
    person_id: PersonId,
) -> HashSet<PersonId> {
    context
        .get_edges::<Person, ET>(person_id)
        .iter()
        .map(|e| e.neighbor)
        .collect()
}

pub fn init(context: &mut Context, people: &[PersonId]) {
    // Create dense household networks
    create_household_networks(context, people);

    // Add U5 edges from csv
    load_edge_list(context, "AgeUnder5Edges.csv", AgeUnder5);

    // Add U18 edges from csv
    load_edge_list(context, "Age5to17Edges.csv", Age5to17);
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
        let deg11 = context.find_entities_by_degree::<Person, Household>(11);
        assert_eq!(deg11.len(), 12 * N_SIZE_12);
    }

    #[test]
    fn test_expected_11_member_household() {
        let mut context = Context::new();
        context.init_random(42);
        let people = loader::init(&mut context);
        network::init(&mut context, &people);
        let deg10 = context.find_entities_by_degree::<Person, Household>(10);
        assert_eq!(deg10.len(), 11 * N_SIZE_11);
    }

    #[test]
    fn test_expected_3_member_household() {
        let mut context = Context::new();
        context.init_random(42);
        let people = loader::init(&mut context);
        network::init(&mut context, &people);
        let deg10 = context.find_entities_by_degree::<Person, Household>(2);
        assert_eq!(deg10.len(), 3 * N_SIZE_3);
    }
}

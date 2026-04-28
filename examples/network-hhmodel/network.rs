use ixa::prelude::*;
use ixa::{HashSet, HashSetExt};
use rand_distr::Bernoulli;

use crate::loader::{open_csv, HouseholdId, Id};
use crate::parameters::Parameters;
use crate::PersonId;

define_entity!(Edge);
// relative transmission rate
define_property!(struct RR(f64), Edge);
define_property!(struct Node1(PersonId), Edge);
define_property!(struct Node2(PersonId), Edge);

define_rng!(NetworkRng);

fn add_bidi_edge(context: &mut Context, p1: PersonId, p2: PersonId, rr: RR) {
    context
        .add_entity(with!(Edge, Node1(p1), Node2(p2), rr))
        .unwrap();
    context
        .add_entity(with!(Edge, Node2(p1), Node1(p2), rr))
        .unwrap();
}

fn create_household_networks(context: &mut Context, people: &[PersonId], rr: RR) {
    let mut households = HashSet::new();
    for person_id in people {
        let household_id: HouseholdId = context.get_property(*person_id);
        if households.insert(household_id) {
            let mut members: Vec<PersonId> = Vec::new();
            context.with_query_results((household_id,), &mut |results| {
                members = results.to_owned_vec()
            });
            // create a dense network
            while let Some(person) = members.pop() {
                for other_person in &members {
                    add_bidi_edge(context, person, *other_person, rr);
                }
            }
        }
    }
}

fn load_edge_list(context: &mut Context, file_name: &str, rr: RR) {
    let mut reader = open_csv(file_name);

    for result in reader.deserialize() {
        let record: (u16, u16) = result.expect("Failed to parse edge");
        let mut p1_vec = Vec::new();
        context.with_query_results((Id(record.0),), &mut |people| {
            p1_vec = people.to_owned_vec()
        });
        assert_eq!(p1_vec.len(), 1);
        let p1 = p1_vec[0];
        let mut p2_vec = Vec::new();
        context.with_query_results((Id(record.1),), &mut |people| {
            p2_vec = people.to_owned_vec()
        });
        assert_eq!(p2_vec.len(), 1);
        let p2 = p2_vec[0];
        add_bidi_edge(context, p1, p2, rr);
    }
}

fn sar_to_beta(sar: f64, infectious_period: f64) -> f64 {
    1.0 - (1.0 - sar).powf(1.0 / infectious_period)
}

/// Get all the effective contacts a person will have over a certain duration
pub fn get_contacts(context: &Context, person_id: PersonId, duration: f64) -> HashSet<PersonId> {
    let parameters = context
        .get_global_property_value(Parameters)
        .unwrap()
        .clone();

    // Base probability of contact during the duration. Note that this assumes that the duration is not too high!
    let base_p = duration * sar_to_beta(parameters.sar, parameters.incubation_period);

    context
        // get all people this person could contact
        .query_result_iterator(with!(Edge, Node1(person_id)))
        .filter_map(|e| {
            // extract risk ratio of transmission and contactee from edge
            let rr: RR = context.get_property(e);
            let node2: Node2 = context.get_property(e);

            // check if they actually make contact
            match context.sample_distr(NetworkRng, Bernoulli::new(base_p * rr.0).unwrap()) {
                false => None,
                true => Some(node2.0),
            }
        })
        .collect()
}

pub fn init(context: &mut Context, people: &[PersonId]) {
    let parameters = context
        .get_global_property_value(Parameters)
        .unwrap()
        .clone();

    // relative risk of transmission between (vs. within) households
    let rr = 1.0 / parameters.between_hh_transmission_reduction;

    // Create dense household networks
    create_household_networks(context, people, RR(1.0));
    // Add other edges from csv's with lower transmission rate
    load_edge_list(context, "AgeUnder5Edges.csv", RR(rr));
    load_edge_list(context, "Age5to17Edges.csv", RR(rr));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{loader, network, Person};

    // Assert that person with `id` has `n` contacts (i.e., edges going from
    // them, and also edges going to them)
    fn assert_has_n_contacts(id: u16, n: usize) {
        let mut context = Context::new();
        context.init_random(42);
        let people = loader::init(&mut context);
        network::init(&mut context, &people);

        // `id` is the data ID, the one in the csv's
        // `pid` is the integer inside Person(pid)
        let person: Person = context.query(with!(Person, Id(id))).into_iter().next();
        let pid: usize = person.id();
        let n_to = context.query_entity_count(with!(Edge, Node1(pid)));
        let n_from = context.query_entity_count(with!(Edge, Node2(pid)));
        assert_eq!(n_to, n);
        assert_eq!(n_from, n);
    }

    #[test]
    fn test_person_826() {
        // Person 826 is in a household of 5 with no other contacts.
        // There should be 4 edges going from them, and 4 going to them.
        assert_has_n_contacts(826, 4);
    }

    #[test]
    fn test_person_243() {
        // Person 243 is in a household of size 5, with 4 other contacts,
        // for 8 total contacts.
        assert_has_n_contacts(243, 8);
    }
}

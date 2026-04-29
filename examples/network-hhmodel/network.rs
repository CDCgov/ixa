use ixa::prelude::*;
use ixa::{HashSet, HashSetExt};
use rand_distr::Bernoulli;

use crate::loader::{open_csv, HouseholdId, Id};
use crate::parameters::Parameters;
use crate::Person;

define_entity!(Edge);
// relative transmission rate
define_property!(struct RR(f64), Edge);
define_property!(struct Node1(EntityId<Person>), Edge);
define_property!(struct Node2(EntityId<Person>), Edge);

define_rng!(NetworkRng);

fn add_bidi_edge(context: &mut Context, p1: EntityId<Person>, p2: EntityId<Person>, rr: RR) {
    context
        .add_entity((Node1(p1), Node2(p2), rr))
        .unwrap();
    context
        .add_entity((Node1(p2), Node2(p1), rr))
        .unwrap();
}

fn create_household_networks(context: &mut Context, rr: RR) {
    let mut households = HashSet::new();
    let people: Vec<EntityId<Person>> = context.get_entity_iterator().collect();
    for person in people {
        let household_id: HouseholdId = context.get_property(person);
        if households.insert(household_id) {
            let members: Vec<EntityId<Person>> =  context.query((household_id,)).into_iter().collect();

            // create a dense network
            for i in 0..(members.len() - 1) {
                for j in (i + 1)..members.len() {
                    assert!(i != j);
                    add_bidi_edge(context, members[i], members[j], rr);
                }
            }
        }
    }
}

fn load_edge_list(context: &mut Context, file_name: &str, rr: RR) {
    let mut reader = open_csv(file_name);

    for result in reader.deserialize() {
        let record: (u16, u16) = result.expect("Failed to parse edge");
        let p1_vec: Vec<EntityId<Person>> = context.query(((Id(record.0)),)).into_iter().collect();
        assert_eq!(p1_vec.len(), 1);
        let p2_vec: Vec<EntityId<Person>> = context.query(((Id(record.1)),)).into_iter().collect();
        assert_eq!(p2_vec.len(), 1);
        add_bidi_edge(context, p1_vec[0], p2_vec[0], rr);
    }
}

fn sar_to_beta(sar: f64, infectious_period: f64) -> f64 {
    1.0 - (1.0 - sar).powf(1.0 / infectious_period)
}

/// Get all the effective contacts a person will have over a certain duration
pub fn get_contacts(context: &Context, person: EntityId<Person>, duration: f64) -> Vec<EntityId<Person>> {
    let parameters = context
        .get_global_property_value(Parameters)
        .unwrap()
        .clone();

    // Base probability of contact during the duration. Note that this assumes that the duration is not too high!
    let base_p = duration * sar_to_beta(parameters.sar, parameters.incubation_period);

    let mut contacts: Vec<EntityId<Person>> = Vec::new();

    for edge in context.query((Node1(person), )) {
        let RR(rr): RR = context.get_property(edge);
        let Node2(person2): Node2 = context.get_property(edge);

        if context.sample_distr(NetworkRng, Bernoulli::new(base_p * rr).unwrap()) {
            if !contacts.contains(&person2) {
                contacts.push(person2);
            }
        }
    }

    contacts
}

pub fn init(context: &mut Context) {
    let parameters = context
        .get_global_property_value(Parameters)
        .unwrap()
        .clone();

    // relative risk of transmission between (vs. within) households
    let rr = 1.0 / parameters.between_hh_transmission_reduction;

    // Create dense household networks
    create_household_networks(context, RR(1.0));
    // Add other edges from csv's with lower transmission rate
    load_edge_list(context, "AgeUnder5Edges.csv", RR(rr));
    load_edge_list(context, "Age5to17Edges.csv", RR(rr));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{loader, network, Person};
    use crate::parameters::ParametersValues;

    // Assert that person with `id` has `n` contacts (i.e., edges going from
    // them, and also edges going to them)
    fn assert_has_n_contacts(id: u16, n: usize) {
        let mut context = Context::new();
        context.init_random(42);
        loader::init(&mut context);
        let parameters = ParametersValues {
            incubation_period: 8.0,
            infectious_period: 27.0,
            sar: 1.0,
            shape: 15.0,
            infection_duration: 5.0,
            between_hh_transmission_reduction: 1.0,
            data_dir: "examples/network-hhmodel/tests".to_owned(),
            output_dir: "examples/network-hhmodel/tests".to_owned(),
        };
        context
            .set_global_property_value(Parameters, parameters)
            .unwrap();
        network::init(&mut context);

        let person = context.query((Id(id),)).into_iter().next().unwrap();
        let n_to = context.query_entity_count((Node1(person),));
        let n_from = context.query_entity_count((Node2(person), ));
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
        // Person 243 is in a household of size 6, with 3 other contacts,
        // for 9 total contacts.
        assert_has_n_contacts(243, 9);
    }
}

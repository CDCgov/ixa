use std::hash::{Hash, Hasher};

use ixa::prelude::*;
use ixa::{HashSet, HashSetExt};
use rand_distr::Bernoulli;
use serde::Serialize;

use crate::loader::{open_csv, HouseholdId, Id};
use crate::parameters::Parameters;
use crate::{Person, PersonId};

// ixa properties must implement `Eq` and `Hash`, but `f64`
// does not. This example manually implements that logic manually.
#[derive(Copy, Clone, Serialize, Debug)]
pub struct FloatEq(f64);

impl PartialEq for FloatEq {
    fn eq(&self, other: &Self) -> bool {
        (self.0.is_nan() && other.0.is_nan()) || (self.0 == other.0)
    }
}

impl Hash for FloatEq {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.to_bits().hash(state);
    }
}

impl Eq for FloatEq {}

define_entity!(Edge);
// relative transmission rate
define_property!(struct RR(FloatEq), Edge);
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

fn create_household_networks(context: &mut Context, rr: RR) {
    let mut households = HashSet::new();
    let people: Vec<PersonId> = context.query(with!(Person)).into_iter().collect();

    // for every person, check what household they are in
    for person_id in people {
        let household_id: HouseholdId = context.get_property(person_id);
        // if we haven't seen this household before, find all its member,
        // and connect them in a dense network
        if households.insert(household_id) {
            let members: Vec<PersonId> = context
                .query(with!(Person, household_id))
                .into_iter()
                .collect();

            for i in 0..(members.len() - 1) {
                for j in (i + 1)..(members.len()) {
                    add_bidi_edge(context, members[i], members[j], rr);
                }
            }
        }
    }
}

// Assert there is only one person with data ID `id`, then get their entity ID
fn get_entity_id_by_data_id(context: &mut Context, id: u16) -> PersonId {
    let v: Vec<PersonId> = context.query(with!(Person, Id(id))).into_iter().collect();
    assert_eq!(v.len(), 1);
    v[0]
}

fn load_edge_list(context: &mut Context, file_name: &str, rr: RR) {
    let mut reader = open_csv(file_name);

    for result in reader.deserialize() {
        let record: (u16, u16) = result.expect("Failed to parse edge");
        let p1 = get_entity_id_by_data_id(context, record.0);
        let p2 = get_entity_id_by_data_id(context, record.1);
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

    // Find all the people this person has edges to. Those people are contacts in this
    // duration, with a certain probability
    let mut contacts = HashSet::new();
    for edge in context.query(with!(Edge, Node1(person_id))) {
        let RR(FloatEq(rr)) = context.get_property(edge);
        let Node2(person2) = context.get_property(edge);
        if context.sample_distr(NetworkRng, Bernoulli::new(base_p * rr).unwrap()) {
            contacts.insert(person2);
        }
    }

    contacts
}

pub fn init(context: &mut Context, between_hh_transmission_reduction: f64) {
    // relative rate of transmission between (vs. within) households
    let rr = 1.0 / between_hh_transmission_reduction;

    // Create dense household networks
    create_household_networks(context, RR(FloatEq(1.0)));
    // Add other edges from csv's with lower transmission rate
    load_edge_list(context, "AgeUnder5Edges.csv", RR(FloatEq(rr)));
    load_edge_list(context, "Age5to17Edges.csv", RR(FloatEq(rr)));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{loader, network};

    // Assert that person with `id` has `n` contacts (i.e., edges going from
    // them, and also edges going to them)
    fn assert_has_n_contacts(id: u16, n: usize) {
        let mut context = Context::new();
        loader::init(&mut context);
        network::init(&mut context, 1.0);

        let eid = get_entity_id_by_data_id(&mut context, id);

        let n_to = context.query_entity_count(with!(Edge, Node1(eid)));
        let n_from = context.query_entity_count(with!(Edge, Node2(eid)));
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
        // Person 243 is in a household of size 6 (i.e., 5 hh contacts)
        // and has 4 other contacts
        assert_has_n_contacts(243, 5 + 4);
    }
}

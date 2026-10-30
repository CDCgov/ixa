use ixa::prelude::*;
use ixa::{HashSet, HashSetExt};

use crate::loader::{open_csv, HouseholdId, Id};
use crate::parameters::Parameters;
use crate::{Person, PersonId};

// One layer per setting. The edge data is the transmission rate relative to a
// household contact, so a pair who share a household and a school has one edge in
// each layer.
define_network!(Contact, Person, layers = { Household, AgeUnder5, Age5to17 }, data = f64);

define_rng!(NetworkRng);

fn create_household_networks(context: &mut Context) {
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
                    // by definition, edge rates are measured relative to within-household rates
                    context
                        .add_edge_bidi(Contact::Household, members[i], members[j], 1.0)
                        .unwrap();
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

fn load_edge_list(context: &mut Context, layer: Contact, file_name: &str, relative_rate: f64) {
    let mut reader = open_csv(file_name);

    for result in reader.deserialize() {
        let record: (u16, u16) = result.expect("Failed to parse edge");
        let p1 = get_entity_id_by_data_id(context, record.0);
        let p2 = get_entity_id_by_data_id(context, record.1);
        context.add_edge_bidi(layer, p1, p2, relative_rate).unwrap();
    }
}

// Assuming that time moves in steps of duration, what is the per-step probability of transmission?
fn sar_to_prob(sar: f64, infectious_period: f64, duration: f64) -> f64 {
    1.0 - (1.0 - sar).powf(duration / infectious_period)
}

/// Get all the effective contacts a person will have over a certain duration
pub fn get_contacts(context: &Context, person_id: PersonId, duration: f64) -> HashSet<PersonId> {
    let parameters = context.get_global_property_value(Parameters).unwrap();

    // Base probability of transmission during the duration.
    let base_prob = duration * sar_to_prob(parameters.sar, parameters.incubation_period, duration);
    assert!(base_prob <= 1.0);

    // Every neighbor in every layer is a contact in this duration with a probability
    // scaled by the edge's relative rate.
    let mut contacts = HashSet::new();
    for &layer in Contact::ALL {
        for edge in context.get_edges(layer, person_id) {
            if context.sample_bool(NetworkRng, base_prob * edge.data) {
                contacts.insert(edge.neighbor);
            }
        }
    }

    contacts
}

// `rr`: relative rate of transmission between (vs. within) households
pub fn init(context: &mut Context, relative_rate: f64) {
    // Rates are relative to a household contact, whose rate is 1, so anything above 1
    // would give a transmission probability above 1.
    assert!(
        (0.0..=1.0).contains(&relative_rate),
        "relative_rate must be in [0, 1]"
    );
    // Create dense household networks
    create_household_networks(context);
    // Add other edges from csv's with lower transmission rate
    load_edge_list(
        context,
        Contact::AgeUnder5,
        "AgeUnder5Edges.csv",
        relative_rate,
    );
    load_edge_list(
        context,
        Contact::Age5to17,
        "Age5to17Edges.csv",
        relative_rate,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{loader, network};

    // Assert that person with `id` has `n` contacts (i.e., edges going from
    // them, and also edges going to them) summed over all layers
    fn assert_has_n_contacts(id: u16, n: usize) {
        let mut context = Context::new();
        loader::init(&mut context);
        network::init(&mut context, 1.0);

        let eid = get_entity_id_by_data_id(&mut context, id);

        let n_to: usize = Contact::ALL
            .iter()
            .map(|&layer| context.get_degree(layer, eid))
            .sum();
        let n_from = context
            .query(with!(Person))
            .into_iter()
            .flat_map(|other| Contact::ALL.iter().map(move |&layer| (layer, other)))
            .filter(|&(layer, other)| context.get_edge(layer, other, eid).is_some())
            .count();
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

    #[test]
    fn test_layers() {
        let mut context = Context::new();
        loader::init(&mut context);
        network::init(&mut context, 0.5);

        let eid = get_entity_id_by_data_id(&mut context, 243);
        assert_eq!(context.get_degree(Contact::Household, eid), 5);
        assert_eq!(
            context.get_degree(Contact::AgeUnder5, eid)
                + context.get_degree(Contact::Age5to17, eid),
            4
        );
        for edge in context.get_edges(Contact::Household, eid) {
            assert_eq!(edge.data, 1.0);
        }
        for edge in context.get_edges(Contact::Age5to17, eid) {
            assert_eq!(edge.data, 0.5);
        }

        // Closing schools removes that layer only.
        context.clear_layer(Contact::Age5to17);
        assert_eq!(context.get_degree(Contact::Age5to17, eid), 0);
        assert_eq!(context.get_degree(Contact::Household, eid), 5);
    }
}

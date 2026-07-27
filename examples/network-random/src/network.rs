use std::hash::Hash;

use ixa::prelude::*;
use rust_igraph::Graph;

use crate::{Person, PersonId};

define_entity!(Edge);
define_property!(struct Node1(PersonId), Edge);
define_property!(struct Node2(PersonId), Edge);

pub fn get_connections(context: &Context, person_id: PersonId) -> Vec<PersonId> {
    context
        .query(with!(Edge, Node1(person_id)))
        .into_iter()
        .map(|edge| context.get_property::<Edge, Node2>(edge).0)
        .collect()
}

pub fn instantiate_person_network(context: &mut Context, g: Graph) {
    let n_people = g.vcount();

    let person_ids: Vec<PersonId> = (0..n_people)
        .map(|_| context.add_entity(with!(Person)).unwrap())
        .collect();

    for (from, to) in g.edges() {
        let p1 = person_ids[from as usize];
        let p2 = person_ids[to as usize];
        context
            .add_entity(with!(Edge, Node1(p1), Node2(p2)))
            .unwrap();

        // if the graph is undirected, add the other (directed) edge
        if !g.is_directed() {
            context
                .add_entity(with!(Edge, Node1(p2), Node2(p1)))
                .unwrap();
        }
    }
}

pub fn init(context: &mut Context, population_size: usize, connection_p: f64, seed: u64) {
    // ideally, we could use some ixa-provided rng for this seed
    let g = Graph::erdos_renyi(population_size as u32, connection_p, seed).unwrap();
    instantiate_person_network(context, g);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_instantiate_network() {
        // set up a manual network, with a certain number of edges
        let g = Graph::from_edges(&[(0, 1), (0, 2), (0, 3), (1, 2)], false, Some(6)).unwrap();

        // turn that network in people entities
        let mut context = Context::new();
        instantiate_person_network(&mut context, g);

        // count how many connections each person has
        let mut n_connections: Vec<usize> = context
            .query(with!(Person))
            .into_iter()
            .map(|person_id| get_connections(&context, person_id).len())
            .collect();

        n_connections.sort();

        // we expect:
        // - person 0 has 3 connections
        // - persons 1 & 2 have 2
        // - person 3 has 1
        // - person 4 & 6 have 0
        assert_eq!(n_connections, vec![0, 0, 1, 2, 2, 3]);
    }
}

use std::hash::Hash;

use itertools::Itertools;
use ixa::prelude::*;
use rand::prelude::IteratorRandom;
use rand::{Rng, SeedableRng};

use crate::{Person, PersonId};

define_entity!(Edge);
define_property!(struct Node1(PersonId), Edge);
define_property!(struct Node2(PersonId), Edge);

// ideally, we could access the rng indexed by the type NetworkRng
// define_rng!(NetworkRng);

pub fn get_connections(context: &Context, person_id: PersonId) -> Vec<PersonId> {
    return context
        .query(with!(Edge, Node1(person_id)))
        .into_iter()
        .map(|edge| context.get_property::<Edge, Node2>(edge).0)
        .collect();
}

pub fn create_gnm_network(rng: &mut impl Rng, n: usize, m: usize) -> Vec<Vec<usize>> {
    if n == 0 {
        assert!(m == 0);
        return vec![];
    }
    assert!(2 * m <= n * (n - 1));
    return (0..n).combinations(2).choose_multiple(rng, m);
}

pub fn instantiate_person_network(context: &mut Context, size: usize, edges: Vec<Vec<usize>>) {
    let person_ids: Vec<PersonId> = (0..size)
        .map(|_| context.add_entity(with!(Person)).unwrap())
        .collect();

    for nodes in edges {
        assert!(nodes.len() == 2);
        let p1 = person_ids[nodes[0]];
        let p2 = person_ids[nodes[1]];
        context
            .add_entity(with!(Edge, Node1(p1), Node2(p2)))
            .unwrap();
    }
}

pub fn init(context: &mut Context, population_size: usize, n_connections: usize, seed: u64) {
    // ideally, we could use the common NetworkRng
    let mut rng = rand::rngs::SmallRng::seed_from_u64(seed);
    let edges = create_gnm_network(&mut rng, population_size, n_connections);
    instantiate_person_network(context, population_size, edges);
}

#[cfg(test)]
mod tests {
    use rand::rngs::SmallRng;

    use super::*;

    fn make_rng() -> SmallRng {
        return SmallRng::seed_from_u64(4824);
    }

    #[test]
    fn test_create_gnm_trivial() {
        let mut rng = make_rng();
        let edges = create_gnm_network(&mut rng, 0, 0);
        assert_eq!(edges.len(), 0)
    }

    #[test]
    fn test_create_gnm_random() {
        let mut rng = make_rng();
        let edges = create_gnm_network(&mut rng, 100, 10);
        assert_eq!(edges.len(), 10);
    }

    #[test]
    fn test_instantiate_network() {
        // set up a manual network, with a certain number of edges
        let size = 6;
        let mut edges = Vec::new();
        for (i, j) in vec![(0, 1), (0, 2), (0, 3), (1, 2)] {
            edges.push(vec![i, j]);
            edges.push(vec![j, i])
        }

        // turn that network in people entities
        let mut context = Context::new();
        instantiate_person_network(&mut context, size, edges);

        // count how many connections each person has
        let n_connections: Vec<usize> = context
            .query(with!(Person))
            .into_iter()
            .map(|person_id| get_connections(&context, person_id).len())
            .sorted()
            .collect();

        // we expect:
        // - person 0 has 3 connections
        // - persons 1 & 2 have 2
        // - person 3 has 1
        // - person 4 & 6 have 0
        assert_eq!(n_connections, vec![0, 0, 1, 2, 2, 3]);
    }
}

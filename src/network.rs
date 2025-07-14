//! A module for modeling contact networks.
//!
//! A network is modeled as a directed graph.  Edges are typed in the
//! usual fashion, i.e., keyed by a Rust type, and each person can have an
//! arbitrary number of outgoing edges of a given type, with each edge
//! having a weight. Edge types can also specify their own per-type
//! data which will be stored along with the edge.
use crate::{
    context::Context, define_data_plugin, error::IxaError, people::PersonId,
    random::ContextRandomExt, random::RngId, HashMap, PluginContext,
};
use rand::Rng;
use std::any::{Any, TypeId};

#[derive(Copy, Clone, Debug, PartialEq)]
/// An edge in network graph. Edges are directed, so the
/// source person is implicit.
pub struct Edge<T: Sized> {
    /// The person this edge comes from.
    pub person: PersonId,
    /// The person this edge points to.
    pub neighbor: PersonId,
    /// The weight associated with the edge.
    pub weight: f32,
    /// An inner value defined by type `T`.
    pub inner: T,
}

pub trait EdgeType {
    type Value: Sized + Default + Copy;
}

#[derive(Default)]
struct PersonNetwork {
    // A vector of vectors of NetworkEdge, indexed by edge type.
    neighbors: HashMap<TypeId, Box<dyn Any>>,
}

struct NetworkData {
    network: Vec<PersonNetwork>,
}

impl NetworkData {
    fn new() -> Self {
        NetworkData {
            network: Vec::new(),
        }
    }

    fn add_edge<T: EdgeType + 'static>(
        &mut self,
        person: PersonId,
        neighbor: PersonId,
        weight: f32,
        inner: T::Value,
    ) -> Result<(), IxaError> {
        if person == neighbor {
            return Err(IxaError::IxaError(String::from("Cannot make edge to self")));
        }

        if weight.is_infinite() || weight.is_nan() || weight.is_sign_negative() {
            return Err(IxaError::IxaError(String::from("Invalid weight")));
        }

        // Make sure we have data for this person.
        if person.0 >= self.network.len() {
            self.network.resize_with(person.0 + 1, Default::default);
        }

        let entry = self.network[person.0]
            .neighbors
            .entry(TypeId::of::<T>())
            .or_insert_with(|| Box::new(Vec::<Edge<T::Value>>::new()));
        let edges: &mut Vec<Edge<T::Value>> = entry.downcast_mut().expect("Type mismatch");

        for edge in edges.iter_mut() {
            if edge.neighbor == neighbor {
                return Err(IxaError::IxaError(String::from("Edge already exists")));
            }
        }

        edges.push(Edge {
            person,
            neighbor,
            weight,
            inner,
        });
        Ok(())
    }

    fn remove_edge<T: EdgeType + 'static>(
        &mut self,
        person: PersonId,
        neighbor: PersonId,
    ) -> Result<(), IxaError> {
        if person.0 >= self.network.len() {
            return Err(IxaError::IxaError(String::from("Edge does not exist")));
        }

        let entry = match self.network[person.0].neighbors.get_mut(&TypeId::of::<T>()) {
            None => {
                return Err(IxaError::IxaError(String::from("Edge does not exist")));
            }
            Some(entry) => entry,
        };

        let edges: &mut Vec<Edge<T::Value>> = entry.downcast_mut().expect("Type mismatch");
        for index in 0..edges.len() {
            if edges[index].neighbor == neighbor {
                edges.remove(index);
                return Ok(());
            }
        }

        Err(IxaError::IxaError(String::from("Edge does not exist")))
    }

    fn get_edge<T: EdgeType + 'static>(
        &self,
        person: PersonId,
        neighbor: PersonId,
    ) -> Option<&Edge<T::Value>> {
        if person.0 >= self.network.len() {
            return None;
        }

        let entry = self.network[person.0].neighbors.get(&TypeId::of::<T>())?;
        let edges: &Vec<Edge<T::Value>> = entry.downcast_ref().expect("Type mismatch");
        edges.iter().find(|&edge| edge.neighbor == neighbor)
    }

    fn get_edges<T: EdgeType + 'static>(&self, person: PersonId) -> Vec<Edge<T::Value>> {
        if person.0 >= self.network.len() {
            return Vec::new();
        }

        let entry = self.network[person.0].neighbors.get(&TypeId::of::<T>());
        if entry.is_none() {
            return Vec::new();
        }

        let edges: &Vec<Edge<T::Value>> = entry.unwrap().downcast_ref().expect("Type mismatch");
        edges.clone()
    }

    fn find_people_by_degree<T: EdgeType + 'static>(&self, degree: usize) -> Vec<PersonId> {
        let mut result = Vec::new();

        for person_id in 0..self.network.len() {
            let entry = self.network[person_id].neighbors.get(&TypeId::of::<T>());
            if entry.is_none() {
                continue;
            }
            let edges: &Vec<Edge<T::Value>> = entry.unwrap().downcast_ref().expect("Type mismatch");
            if edges.len() == degree {
                result.push(PersonId(person_id));
            }
        }
        result
    }
}

/// Define a new edge type for use with `network`.
///
/// Defines a new edge type of type `$edge_type`, with inner type `$value`.
/// Use `()` for `$value` to have no inner type.
#[allow(unused_macros)]
#[macro_export]
macro_rules! define_edge_type {
    ($edge_type:ident, $value:ty) => {
        #[derive(Debug, Copy, Clone)]
        pub struct $edge_type;

        impl $crate::network::EdgeType for $edge_type {
            type Value = $value;
        }
    };
}

define_data_plugin!(NetworkPlugin, NetworkData, NetworkData::new());

// Public API.
pub trait ContextNetworkExt: PluginContext + ContextRandomExt {
    /// Add an edge of type `T` between `person` and `neighbor` with a
    /// given `weight`.  `inner` is a value of whatever type is
    /// associated with `T`.
    ///
    /// # Errors
    ///
    /// Returns `IxaError` if:
    ///
    /// * `person` and `neighbor` are the same or an edge already
    ///   exists between them.
    /// * `weight` is invalid
    fn add_edge<T: EdgeType + 'static>(
        &mut self,
        person: PersonId,
        neighbor: PersonId,
        weight: f32,
        inner: T::Value,
    ) -> Result<(), IxaError> {
        let data_container = self.get_data_container_mut(NetworkPlugin);
        data_container.add_edge::<T>(person, neighbor, weight, inner)
    }

    /// Add a pair of edges of type `T` between `person1` and
    /// `neighbor2` with a given `weight`, one edge in each
    /// direction. `inner` is a value of whatever type is associated
    /// with `T`. This is syntactic sugar for calling `add_edge()`
    /// twice.
    ///
    /// # Errors
    ///
    /// Returns `IxaError` if:
    ///
    /// * `person` and `neighbor` are the same or an edge already
    ///   exists between them.
    /// * `weight` is invalid
    fn add_edge_bidi<T: EdgeType + 'static>(
        &mut self,
        person1: PersonId,
        person2: PersonId,
        weight: f32,
        inner: T::Value,
    ) -> Result<(), IxaError> {
        let data_container = self.get_data_container_mut(NetworkPlugin);
        data_container.add_edge::<T>(person1, person2, weight, inner)?;
        data_container.add_edge::<T>(person2, person1, weight, inner)
    }

    /// Remove an edge of type `T` between `person` and `neighbor`
    /// if one exists.
    ///
    /// # Errors
    /// Returns `IxaError` if no edge exists.
    fn remove_edge<T: EdgeType + 'static>(
        &mut self,
        person: PersonId,
        neighbor: PersonId,
    ) -> Result<(), IxaError> {
        let data_container = self.get_data_container_mut(NetworkPlugin);
        data_container.remove_edge::<T>(person, neighbor)
    }

    /// Get an edge of type `T` between `person` and `neighbor`
    /// if one exists.
    fn get_edge<T: EdgeType + 'static>(
        &self,
        person: PersonId,
        neighbor: PersonId,
    ) -> Option<&Edge<T::Value>> {
        self.get_data_container(NetworkPlugin)
            .get_edge::<T>(person, neighbor)
    }

    /// Get all edges of type `T` from `person`.
    fn get_edges<T: EdgeType + 'static>(&self, person: PersonId) -> Vec<Edge<T::Value>> {
        self.get_data_container(NetworkPlugin)
            .get_edges::<T>(person)
    }

    /// Get all edges of type `T` from `person` that match the predicate
    /// provided in `filter`. Note that because `filter` has access to
    /// both the edge, which contains the neighbor and `Context`, it is
    /// possible to filter on properties of the neighbor. The function
    /// `context.matching_person()` might be helpful here.
    ///
    fn get_matching_edges<T: EdgeType + 'static>(
        &self,
        person: PersonId,
        filter: impl Fn(&Context, &Edge<T::Value>) -> bool + 'static,
    ) -> Vec<Edge<T::Value>>;

    /// Find all people who have an edge of type `T` and degree `degree`.
    fn find_people_by_degree<T: EdgeType + 'static>(&self, degree: usize) -> Vec<PersonId> {
        self.get_data_container(NetworkPlugin)
            .find_people_by_degree::<T>(degree)
    }

    /// Select a random edge out of the list of outgoing edges of type
    /// `T` from `person_id`, weighted by the edge weights.
    ///
    /// # Errors
    /// Returns `IxaError` if there are no edges.
    fn select_random_edge<T: EdgeType + 'static, R: RngId + 'static>(
        &self,
        rng_id: R,
        person_id: PersonId,
    ) -> Result<Edge<T::Value>, IxaError>
    where
        R::RngType: Rng,
    {
        let edges = self.get_edges::<T>(person_id);
        if edges.is_empty() {
            return Err(IxaError::IxaError(String::from(
                "Can't sample from empty list",
            )));
        }

        let weights: Vec<_> = edges.iter().map(|x| x.weight).collect();
        let index = self.sample_weighted(rng_id, &weights);
        Ok(edges[index])
    }
}
impl ContextNetworkExt for Context {
    fn get_matching_edges<T: EdgeType + 'static>(
        &self,
        person: PersonId,
        filter: impl Fn(&Context, &Edge<T::Value>) -> bool + 'static,
    ) -> Vec<Edge<T::Value>> {
        let edges = self.get_edges::<T>(person);
        let mut result = Vec::new();
        for edge in &edges {
            if filter(self, edge) {
                result.push(*edge);
            }
        }
        result
    }
}

#[cfg(test)]
#[allow(clippy::float_cmp)]
// Tests for the inner core.
mod test_inner {
    use super::{Edge, NetworkData};
    use crate::error::IxaError;
    use crate::people::PersonId;

    define_edge_type!(EdgeType1, ());
    define_edge_type!(EdgeType2, ());
    define_edge_type!(EdgeType3, bool);

    #[test]
    fn add_edge() {
        let mut nd = NetworkData::new();

        nd.add_edge::<EdgeType1>(PersonId(1), PersonId(2), 0.01, ())
            .unwrap();
        let edge = nd.get_edge::<EdgeType1>(PersonId(1), PersonId(2)).unwrap();
        assert_eq!(edge.weight, 0.01);
    }

    #[test]
    fn add_edge_with_inner() {
        let mut nd = NetworkData::new();

        nd.add_edge::<EdgeType3>(PersonId(1), PersonId(2), 0.01, true)
            .unwrap();
        let edge = nd.get_edge::<EdgeType3>(PersonId(1), PersonId(2)).unwrap();
        assert_eq!(edge.weight, 0.01);
        assert!(edge.inner);
    }

    #[test]
    fn add_two_edges() {
        let mut nd = NetworkData::new();

        nd.add_edge::<EdgeType1>(PersonId(1), PersonId(2), 0.01, ())
            .unwrap();
        nd.add_edge::<EdgeType1>(PersonId(1), PersonId(3), 0.02, ())
            .unwrap();
        let edge = nd.get_edge::<EdgeType1>(PersonId(1), PersonId(2)).unwrap();
        assert_eq!(edge.weight, 0.01);
        let edge = nd.get_edge::<EdgeType1>(PersonId(1), PersonId(3)).unwrap();
        assert_eq!(edge.weight, 0.02);

        let edges = nd.get_edges::<EdgeType1>(PersonId(1));
        assert_eq!(
            edges,
            vec![
                Edge {
                    person: PersonId(1),
                    neighbor: PersonId(2),
                    weight: 0.01,
                    inner: ()
                },
                Edge {
                    person: PersonId(1),
                    neighbor: PersonId(3),
                    weight: 0.02,
                    inner: ()
                }
            ]
        );
    }

    #[test]
    fn add_two_edge_types() {
        let mut nd = NetworkData::new();

        nd.add_edge::<EdgeType1>(PersonId(1), PersonId(2), 0.01, ())
            .unwrap();
        nd.add_edge::<EdgeType2>(PersonId(1), PersonId(2), 0.02, ())
            .unwrap();
        let edge = nd.get_edge::<EdgeType1>(PersonId(1), PersonId(2)).unwrap();
        assert_eq!(edge.weight, 0.01);
        let edge = nd.get_edge::<EdgeType2>(PersonId(1), PersonId(2)).unwrap();
        assert_eq!(edge.weight, 0.02);

        let edges = nd.get_edges::<EdgeType1>(PersonId(1));
        assert_eq!(
            edges,
            vec![Edge {
                person: PersonId(1),
                neighbor: PersonId(2),
                weight: 0.01,
                inner: ()
            }]
        );
    }

    #[test]
    fn add_edge_twice_fails() {
        let mut nd = NetworkData::new();

        nd.add_edge::<EdgeType1>(PersonId(1), PersonId(2), 0.01, ())
            .unwrap();
        let edge = nd.get_edge::<EdgeType1>(PersonId(1), PersonId(2)).unwrap();
        assert_eq!(edge.weight, 0.01);

        assert!(matches!(
            nd.add_edge::<EdgeType1>(PersonId(1), PersonId(2), 0.02, ()),
            Err(IxaError::IxaError(_))
        ));
    }

    #[test]
    fn add_remove_add_edge() {
        let mut nd = NetworkData::new();

        nd.add_edge::<EdgeType1>(PersonId(1), PersonId(2), 0.01, ())
            .unwrap();
        let edge = nd.get_edge::<EdgeType1>(PersonId(1), PersonId(2)).unwrap();
        assert_eq!(edge.weight, 0.01);

        nd.remove_edge::<EdgeType1>(PersonId(1), PersonId(2))
            .unwrap();
        let edge = nd.get_edge::<EdgeType1>(PersonId(1), PersonId(2));
        assert!(edge.is_none());

        nd.add_edge::<EdgeType1>(PersonId(1), PersonId(2), 0.02, ())
            .unwrap();
        let edge = nd.get_edge::<EdgeType1>(PersonId(1), PersonId(2)).unwrap();
        assert_eq!(edge.weight, 0.02);
    }

    #[test]
    fn remove_nonexistent_edge() {
        let mut nd = NetworkData::new();
        assert!(matches!(
            nd.remove_edge::<EdgeType1>(PersonId(1), PersonId(2)),
            Err(IxaError::IxaError(_))
        ));
    }

    #[test]
    fn add_edge_to_self() {
        let mut nd = NetworkData::new();

        let result = nd.add_edge::<EdgeType1>(PersonId(1), PersonId(1), 0.01, ());
        assert!(matches!(result, Err(IxaError::IxaError(_))));
    }

    #[test]
    fn add_edge_bogus_weight() {
        let mut nd = NetworkData::new();

        let result = nd.add_edge::<EdgeType1>(PersonId(1), PersonId(2), -1.0, ());
        assert!(matches!(result, Err(IxaError::IxaError(_))));

        let result = nd.add_edge::<EdgeType1>(PersonId(1), PersonId(2), f32::NAN, ());
        assert!(matches!(result, Err(IxaError::IxaError(_))));

        let result = nd.add_edge::<EdgeType1>(PersonId(1), PersonId(2), f32::INFINITY, ());
        assert!(matches!(result, Err(IxaError::IxaError(_))));
    }

    #[test]
    fn find_people_by_degree() {
        let mut nd = NetworkData::new();

        nd.add_edge::<EdgeType1>(PersonId(1), PersonId(2), 0.0, ())
            .unwrap();
        nd.add_edge::<EdgeType1>(PersonId(1), PersonId(3), 0.0, ())
            .unwrap();
        nd.add_edge::<EdgeType1>(PersonId(2), PersonId(3), 0.0, ())
            .unwrap();
        nd.add_edge::<EdgeType1>(PersonId(3), PersonId(2), 0.0, ())
            .unwrap();

        let matches = nd.find_people_by_degree::<EdgeType1>(2);
        assert_eq!(matches, vec![PersonId(1)]);
        let matches = nd.find_people_by_degree::<EdgeType1>(1);
        assert_eq!(matches, vec![PersonId(2), PersonId(3)]);
    }
}

#[cfg(test)]
#[allow(clippy::float_cmp)]
// Tests for the API.
mod test_api {
    use crate::context::Context;
    use crate::define_rng;
    use crate::error::IxaError;
    use crate::network::{ContextNetworkExt, Edge};
    use crate::people::{define_person_property, ContextPeopleExt, PersonId};
    use crate::random::ContextRandomExt;

    define_edge_type!(EdgeType1, u32);
    define_person_property!(Age, u8);

    fn setup() -> (Context, PersonId, PersonId) {
        let mut context = Context::new();
        let person1 = context.add_person((Age, 1)).unwrap();
        let person2 = context.add_person((Age, 2)).unwrap();

        (context, person1, person2)
    }

    #[test]
    fn add_edge() {
        let (mut context, person1, person2) = setup();

        context
            .add_edge::<EdgeType1>(person1, person2, 0.01, 1)
            .unwrap();
        assert_eq!(
            context
                .get_edge::<EdgeType1>(person1, person2)
                .unwrap()
                .weight,
            0.01
        );
        assert_eq!(
            context.get_edges::<EdgeType1>(person1),
            vec![Edge {
                person: person1,
                neighbor: person2,
                weight: 0.01,
                inner: 1
            }]
        );
    }

    #[test]
    fn remove_edge() {
        let (mut context, person1, person2) = setup();
        // Check that we get an error if nothing has been added.

        assert!(matches!(
            context.remove_edge::<EdgeType1>(person1, person2),
            Err(IxaError::IxaError(_))
        ));
        context
            .add_edge::<EdgeType1>(person1, person2, 0.01, 1)
            .unwrap();
        context.remove_edge::<EdgeType1>(person1, person2).unwrap();
        assert!(context.get_edge::<EdgeType1>(person1, person2).is_none());
        assert_eq!(context.get_edges::<EdgeType1>(person1).len(), 0);
    }

    #[test]
    fn add_edge_bidi() {
        let (mut context, person1, person2) = setup();

        context
            .add_edge_bidi::<EdgeType1>(person1, person2, 0.01, 1)
            .unwrap();
        assert_eq!(
            context
                .get_edge::<EdgeType1>(person1, person2)
                .unwrap()
                .weight,
            0.01
        );
        assert_eq!(
            context
                .get_edge::<EdgeType1>(person2, person1)
                .unwrap()
                .weight,
            0.01
        );
    }

    #[test]
    fn add_edge_different_weights() {
        let (mut context, person1, person2) = setup();

        context
            .add_edge::<EdgeType1>(person1, person2, 0.01, 1)
            .unwrap();
        context
            .add_edge::<EdgeType1>(person2, person1, 0.02, 1)
            .unwrap();
        assert_eq!(
            context
                .get_edge::<EdgeType1>(person1, person2)
                .unwrap()
                .weight,
            0.01
        );
        assert_eq!(
            context
                .get_edge::<EdgeType1>(person2, person1)
                .unwrap()
                .weight,
            0.02
        );
    }

    #[test]
    fn get_matching_edges_weight() {
        let (mut context, person1, person2) = setup();
        let person3 = context.add_person((Age, 3)).unwrap();

        context
            .add_edge::<EdgeType1>(person1, person2, 0.01, 1)
            .unwrap();
        context
            .add_edge::<EdgeType1>(person1, person3, 0.03, 1)
            .unwrap();
        let edges =
            context.get_matching_edges::<EdgeType1>(person1, |_context, edge| edge.weight > 0.01);
        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0].person, person1);
        assert_eq!(edges[0].neighbor, person3);
    }

    #[test]
    fn get_matching_edges_inner() {
        let (mut context, person1, person2) = setup();
        let person3 = context.add_person((Age, 3)).unwrap();

        context
            .add_edge::<EdgeType1>(person1, person2, 0.01, 1)
            .unwrap();
        context
            .add_edge::<EdgeType1>(person1, person3, 0.03, 3)
            .unwrap();
        let edges =
            context.get_matching_edges::<EdgeType1>(person1, |_context, edge| edge.inner == 3);
        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0].person, person1);
        assert_eq!(edges[0].neighbor, person3);
    }

    #[test]
    fn get_matching_edges_person_property() {
        let (mut context, person1, person2) = setup();
        let person3 = context.add_person((Age, 3)).unwrap();

        context
            .add_edge::<EdgeType1>(person1, person2, 0.01, 1)
            .unwrap();
        context
            .add_edge::<EdgeType1>(person1, person3, 0.03, 3)
            .unwrap();
        let edges = context.get_matching_edges::<EdgeType1>(person1, |context, edge| {
            context.match_person(edge.neighbor, (Age, 3))
        });
        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0].person, person1);
        assert_eq!(edges[0].neighbor, person3);
    }

    #[test]
    fn select_random_edge() {
        define_rng!(NetworkTestRng);

        let (mut context, person1, person2) = setup();
        let person3 = context.add_person((Age, 3)).unwrap();
        context.init_random(42);

        context
            .add_edge::<EdgeType1>(person1, person2, 0.01, 1)
            .unwrap();
        context
            .add_edge::<EdgeType1>(person1, person3, 10_000_000.0, 3)
            .unwrap();

        let edge = context
            .select_random_edge::<EdgeType1, _>(NetworkTestRng, person1)
            .unwrap();
        assert_eq!(edge.person, person1);
        assert_eq!(edge.neighbor, person3);
    }
}

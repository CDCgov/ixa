//! A module for modeling contact networks.
//!
//! A network is modeled as a directed graph.  Edges are typed in the
//! usual fashion, i.e., keyed by a Rust type, and each person can have an
//! arbitrary number of outgoing edges of a given type, with each edge
//! having a weight. Edge types can also specify their own per-type
//! data which will be stored along with the edge.

pub mod edge;
mod network;
mod network_store;

use std::any::Any;
use std::cell::OnceCell;

pub use edge::{Edge, EdgeType};
use network::{AdjacencyList, Network};
use network_store::NetworkStore;
use rand::Rng;

use crate::context::Context;
use crate::entity::entity_store::get_registered_entity_count;
use crate::entity::{Entity, EntityId};
use crate::error::IxaError;
use crate::random::{ContextRandomExt, RngId};
use crate::{define_data_plugin, ContextBase};

pub struct NetworkData {
    /// A map from `Entity::id()` to `Box<NetworkStore<E>>`.
    network_stores: Vec<OnceCell<Box<dyn Any>>>,
}

impl Default for NetworkData {
    fn default() -> Self {
        Self::new()
    }
}

impl NetworkData {
    #[must_use]
    pub fn new() -> Self {
        let entity_count = get_registered_entity_count();
        let network_stores = (0..entity_count)
            .map(|_| OnceCell::new())
            .collect::<Vec<_>>();

        NetworkData { network_stores }
    }

    /// Inserts the given edge into the adjacency list for the given entity.
    /// If an edge having the same neighbor as the given edge exists, an error is returned.
    pub fn add_edge<E: Entity, ET: EdgeType<E>>(
        &mut self,
        entity_id: EntityId<E>,
        neighbor: EntityId<E>,
        weight: f32,
        inner: ET,
    ) -> Result<(), IxaError> {
        if entity_id == neighbor {
            return Err(IxaError::CannotMakeEdgeToSelf);
        }
        if weight.is_infinite() || weight.is_nan() || weight.is_sign_negative() {
            return Err(IxaError::InvalidWeight);
        }

        let edge = Edge {
            neighbor,
            weight,
            inner,
        };
        let network = self.get_network_mut::<E, ET>();

        network.add_edge(entity_id, edge)
    }

    /// Remove the edge from the given entity to the given neighbor and return it, or
    /// `None` if the edge does not exist.
    pub fn remove_edge<E: Entity, ET: EdgeType<E>>(
        &mut self,
        entity_id: EntityId<E>,
        neighbor: EntityId<E>,
    ) -> Option<Edge<E, ET>> {
        let network = self.get_network_mut::<E, ET>();
        network.remove_edge(entity_id, neighbor)
    }

    /// Returns an immutable reference to the edge from the given entity
    /// to the given neighbor, or `None` if the edge does not exist.
    #[must_use]
    pub fn get_edge<E: Entity, ET: EdgeType<E>>(
        &self,
        entity_id: EntityId<E>,
        neighbor: EntityId<E>,
    ) -> Option<&Edge<E, ET>> {
        let network = self.get_network::<E, ET>();
        network.get_edge(entity_id, neighbor)
    }

    /// Returns a clone of the adjacency list for the given entity. Returns an empty list
    /// if the list is empty.
    #[must_use]
    pub fn get_edges<E: Entity, ET: EdgeType<E>>(
        &self,
        entity_id: EntityId<E>,
    ) -> AdjacencyList<E, ET> {
        let network = self.get_network::<E, ET>();
        network.get_list_cloned(entity_id)
    }

    /// Returns a clone of the adjacency list for the given entity. Returns an empty list
    /// if the list is empty.
    #[must_use]
    pub fn get_edges_ref<E: Entity, ET: EdgeType<E>>(
        &self,
        entity_id: EntityId<E>,
    ) -> Option<&AdjacencyList<E, ET>> {
        let network = self.get_network::<E, ET>();
        network.get_list(entity_id)
    }

    /// Returns a list of `EntityId<E>`s having exactly the given number of neighbors.
    #[must_use]
    pub fn find_entities_by_degree<E: Entity, ET: EdgeType<E>>(
        &self,
        degree: usize,
    ) -> Vec<EntityId<E>> {
        let network = self.get_network::<E, ET>();
        network.find_entities_by_degree(degree)
    }

    #[must_use]
    fn get_network<E: Entity, ET: EdgeType<E>>(&self) -> &Network<E, ET> {
        self.network_stores
            .get(E::id())
            .unwrap_or_else(|| {
                panic!(
                    "internal error: NetworkStore for Entity {} not found",
                    E::name()
                )
            })
            .get_or_init(NetworkStore::<E>::new_boxed)
            .downcast_ref::<NetworkStore<E>>()
            .unwrap_or_else(|| {
                panic!(
                    "internal error: found wrong NetworkStore type when accessing Entity {}",
                    E::name()
                )
            })
            .get::<ET>()
    }

    #[must_use]
    fn get_network_mut<E: Entity, ET: EdgeType<E>>(&mut self) -> &mut Network<E, ET> {
        let network_store = self.network_stores.get_mut(E::id()).unwrap_or_else(|| {
            panic!(
                "internal error: NetworkStore for Entity {} not found",
                E::name()
            )
        });

        // Lazily initialize the NetworkStore if needed.
        if network_store.get().is_none() {
            network_store.set(NetworkStore::<E>::new_boxed()).unwrap();
        }

        // Now the unwrap on `get_mut` is guaranteed to succeed
        network_store
            .get_mut()
            .unwrap()
            .downcast_mut::<NetworkStore<E>>()
            .unwrap_or_else(|| {
                panic!(
                    "internal error: found wrong NetworkStore type when accessing Entity {}",
                    E::name()
                )
            })
            .get_mut::<ET>()
    }
}

define_data_plugin!(NetworkPlugin, NetworkData, NetworkData::new());

// Public API.
pub trait ContextNetworkExt: ContextBase + ContextRandomExt {
    /// Add an edge of type `ET` between `entity_id` and `neighbor` with a
    /// given `weight`.
    ///
    /// # Errors
    ///
    /// Returns [`IxaError`] if:
    ///
    /// * `entity_id` and `neighbor` are the same or an edge already
    ///   exists between them.
    /// * `weight` is invalid
    fn add_edge<E: Entity, ET: EdgeType<E>>(
        &mut self,
        entity_id: EntityId<E>,
        neighbor: EntityId<E>,
        weight: f32,
        inner: ET,
    ) -> Result<(), IxaError> {
        let data_container = self.get_data_mut(NetworkPlugin);
        data_container.add_edge::<E, ET>(entity_id, neighbor, weight, inner)
    }

    /// Add a pair of edges of type `ET` between `entity1` and `entity2` with a given `weight`,
    /// one edge in each direction. This is syntactic sugar for calling
    /// [`add_edge`](Self::add_edge) twice.
    ///
    /// # Errors
    ///
    /// Returns [`IxaError`] if:
    ///
    /// * `entity1` and `entity2` are the same or an edge already
    ///   exists between them.
    /// * `weight` is invalid
    fn add_edge_bidi<E: Entity, ET: EdgeType<E>>(
        &mut self,
        entity1: EntityId<E>,
        entity2: EntityId<E>,
        weight: f32,
        inner: ET,
    ) -> Result<(), IxaError> {
        let data_container = self.get_data_mut(NetworkPlugin);
        data_container.add_edge::<E, ET>(entity1, entity2, weight, inner.clone())?;
        data_container.add_edge::<E, ET>(entity2, entity1, weight, inner)
    }

    /// Remove the edge of type `ET` from `entity_id` to `neighbor` and return it, or `None` if
    /// the edge does not exist.
    fn remove_edge<E: Entity, ET: EdgeType<E>>(
        &mut self,
        entity_id: EntityId<E>,
        neighbor: EntityId<E>,
    ) -> Option<Edge<E, ET>> {
        let data_container = self.get_data_mut(NetworkPlugin);
        data_container.remove_edge::<E, ET>(entity_id, neighbor)
    }

    /// Get an edge of type `ET` from `entity_id` to `neighbor` if one exists.
    #[must_use]
    fn get_edge<E: Entity, ET: EdgeType<E>>(
        &self,
        entity_id: EntityId<E>,
        neighbor: EntityId<E>,
    ) -> Option<&Edge<E, ET>> {
        self.get_data(NetworkPlugin)
            .get_edge::<E, ET>(entity_id, neighbor)
    }

    /// Get all outgoing edges of type `ET` from `entity_id`.
    #[must_use]
    fn get_edges<E: Entity, ET: EdgeType<E>>(
        &self,
        entity_id: EntityId<E>,
    ) -> AdjacencyList<E, ET> {
        self.get_data(NetworkPlugin).get_edges::<E, ET>(entity_id)
    }

    /// Get all edges of type `ET` from `entity_id` that match the predicate provided in `filter`.
    ///
    /// Note that because `filter` has access to both the edge (which contains `neighbor`) and
    /// the calling context (`self`), it is possible to filter on properties of the neighbor.
    #[must_use]
    fn get_matching_edges<E: Entity, ET: EdgeType<E>>(
        &self,
        entity_id: EntityId<E>,
        filter: impl Fn(&Self, &Edge<E, ET>) -> bool,
    ) -> AdjacencyList<E, ET> {
        let network_data = self.get_data(NetworkPlugin);
        let empty = vec![];
        let edges = network_data
            .get_edges_ref::<E, ET>(entity_id)
            .unwrap_or(&empty);
        edges
            .iter()
            .filter(|&edge| filter(self, edge))
            .cloned()
            .collect()
    }

    /// Find all entities who have an edge of type `ET` and degree `degree`.
    #[must_use]
    fn find_entities_by_degree<E: Entity, ET: EdgeType<E>>(
        &self,
        degree: usize,
    ) -> Vec<EntityId<E>> {
        self.get_data(NetworkPlugin)
            .find_entities_by_degree::<E, ET>(degree)
    }

    /// Select a random outgoing edge of type `ET` from `entity_id`, weighted by the edge weights.
    ///
    /// # Errors
    /// Returns [`IxaError`] if there are no edges.
    fn select_random_edge<E: Entity, ET: EdgeType<E>, R: RngId + 'static>(
        &self,
        rng_id: R,
        entity_id: EntityId<E>,
    ) -> Result<Edge<E, ET>, IxaError>
    where
        R::RngType: Rng,
    {
        let edges = self.get_edges::<E, ET>(entity_id);
        if edges.is_empty() {
            return Err(IxaError::CannotSampleFromEmptyList);
        }

        let weights: Vec<_> = edges.iter().map(|x| x.weight).collect();
        let index = self.sample_weighted(rng_id, &weights);
        Ok(edges[index].clone())
    }
}

impl ContextNetworkExt for Context {}

#[cfg(test)]
#[allow(clippy::float_cmp)]
// Tests for the inner core.
mod test_inner {
    use super::NetworkData;
    use crate::error::IxaError;
    use crate::network::edge::Edge;
    use crate::{define_edge_type, define_entity};

    define_entity!(Person);

    define_edge_type!(struct EdgeType1, Person);
    define_edge_type!(struct EdgeType2, Person);
    define_edge_type!(struct EdgeType3(pub bool), Person);

    #[test]
    fn add_edge() {
        let mut nd = NetworkData::new();

        nd.add_edge::<Person, EdgeType1>(PersonId::new(1), PersonId::new(2), 0.01, EdgeType1)
            .unwrap();
        let edge = nd
            .get_edge::<Person, EdgeType1>(PersonId::new(1), PersonId::new(2))
            .unwrap();
        assert_eq!(edge.weight, 0.01);
    }

    #[test]
    fn add_edge_with_inner() {
        let mut nd = NetworkData::new();

        nd.add_edge::<Person, EdgeType3>(PersonId::new(1), PersonId::new(2), 0.01, EdgeType3(true))
            .unwrap();
        let edge = nd
            .get_edge::<Person, EdgeType3>(PersonId::new(1), PersonId::new(2))
            .unwrap();
        assert_eq!(edge.weight, 0.01);
        assert_eq!(edge.inner, EdgeType3(true));
    }

    #[test]
    fn add_two_edges() {
        let mut nd = NetworkData::new();

        nd.add_edge::<Person, EdgeType1>(PersonId::new(1), PersonId::new(2), 0.01, EdgeType1)
            .unwrap();
        nd.add_edge::<Person, EdgeType1>(PersonId::new(1), PersonId::new(3), 0.02, EdgeType1)
            .unwrap();
        let edge = nd
            .get_edge::<Person, EdgeType1>(PersonId::new(1), PersonId::new(2))
            .unwrap();
        assert_eq!(edge.weight, 0.01);
        let edge = nd
            .get_edge::<Person, EdgeType1>(PersonId::new(1), PersonId::new(3))
            .unwrap();
        assert_eq!(edge.weight, 0.02);

        let edges = nd.get_edges::<Person, EdgeType1>(PersonId::new(1));
        assert_eq!(
            edges,
            vec![
                Edge {
                    neighbor: PersonId::new(2),
                    weight: 0.01,
                    inner: EdgeType1
                },
                Edge {
                    neighbor: PersonId::new(3),
                    weight: 0.02,
                    inner: EdgeType1
                }
            ]
        );
    }

    #[test]
    fn add_two_edge_types() {
        let mut nd = NetworkData::new();

        nd.add_edge::<Person, EdgeType1>(PersonId::new(1), PersonId::new(2), 0.01, EdgeType1)
            .unwrap();
        nd.add_edge::<Person, EdgeType2>(PersonId::new(1), PersonId::new(2), 0.02, EdgeType2)
            .unwrap();
        let edge = nd
            .get_edge::<Person, EdgeType1>(PersonId::new(1), PersonId::new(2))
            .unwrap();
        assert_eq!(edge.weight, 0.01);
        let edge = nd
            .get_edge::<Person, EdgeType2>(PersonId::new(1), PersonId::new(2))
            .unwrap();
        assert_eq!(edge.weight, 0.02);

        let edges = nd.get_edges::<Person, EdgeType1>(PersonId::new(1));
        assert_eq!(
            edges,
            vec![Edge {
                neighbor: PersonId::new(2),
                weight: 0.01,
                inner: EdgeType1
            }]
        );
    }

    #[test]
    fn add_edge_twice_fails() {
        let mut nd = NetworkData::new();

        nd.add_edge::<Person, EdgeType1>(PersonId::new(1), PersonId::new(2), 0.01, EdgeType1)
            .unwrap();
        let edge = nd
            .get_edge::<Person, EdgeType1>(PersonId::new(1), PersonId::new(2))
            .unwrap();
        assert_eq!(edge.weight, 0.01);

        assert!(matches!(
            nd.add_edge::<Person, EdgeType1>(PersonId::new(1), PersonId::new(2), 0.02, EdgeType1),
            Err(IxaError::EdgeAlreadyExists)
        ));
    }

    #[test]
    fn add_remove_add_edge() {
        let mut nd = NetworkData::new();

        nd.add_edge::<Person, EdgeType1>(PersonId::new(1), PersonId::new(2), 0.01, EdgeType1)
            .unwrap();
        let edge = nd
            .get_edge::<Person, EdgeType1>(PersonId::new(1), PersonId::new(2))
            .unwrap();
        assert_eq!(edge.weight, 0.01);

        nd.remove_edge::<Person, EdgeType1>(PersonId::new(1), PersonId::new(2))
            .unwrap();
        let edge = nd.get_edge::<Person, EdgeType1>(PersonId::new(1), PersonId::new(2));
        assert!(edge.is_none());

        nd.add_edge::<Person, EdgeType1>(PersonId::new(1), PersonId::new(2), 0.02, EdgeType1)
            .unwrap();
        let edge = nd
            .get_edge::<Person, EdgeType1>(PersonId::new(1), PersonId::new(2))
            .unwrap();
        assert_eq!(edge.weight, 0.02);
    }

    #[test]
    fn remove_nonexistent_edge() {
        let mut nd = NetworkData::new();
        assert!(nd
            .remove_edge::<Person, EdgeType1>(PersonId::new(1), PersonId::new(2))
            .is_none());
    }

    #[test]
    fn add_edge_to_self() {
        let mut nd = NetworkData::new();

        let result =
            nd.add_edge::<Person, EdgeType1>(PersonId::new(1), PersonId::new(1), 0.01, EdgeType1);
        assert!(matches!(result, Err(IxaError::CannotMakeEdgeToSelf)));
    }

    #[test]
    fn add_edge_bogus_weight() {
        let mut nd = NetworkData::new();

        let result =
            nd.add_edge::<Person, EdgeType1>(PersonId::new(1), PersonId::new(2), -1.0, EdgeType1);
        assert!(matches!(result, Err(IxaError::InvalidWeight)));

        let result = nd.add_edge::<Person, EdgeType1>(
            PersonId::new(1),
            PersonId::new(2),
            f32::NAN,
            EdgeType1,
        );
        assert!(matches!(result, Err(IxaError::InvalidWeight)));

        let result = nd.add_edge::<Person, EdgeType1>(
            PersonId::new(1),
            PersonId::new(2),
            f32::INFINITY,
            EdgeType1,
        );
        assert!(matches!(result, Err(IxaError::InvalidWeight)));
    }

    #[test]
    fn find_people_by_degree() {
        let mut nd = NetworkData::new();

        nd.add_edge::<Person, EdgeType1>(PersonId::new(1), PersonId::new(2), 0.0, EdgeType1)
            .unwrap();
        nd.add_edge::<Person, EdgeType1>(PersonId::new(1), PersonId::new(3), 0.0, EdgeType1)
            .unwrap();
        nd.add_edge::<Person, EdgeType1>(PersonId::new(2), PersonId::new(3), 0.0, EdgeType1)
            .unwrap();
        nd.add_edge::<Person, EdgeType1>(PersonId::new(3), PersonId::new(2), 0.0, EdgeType1)
            .unwrap();

        let matches = nd.find_entities_by_degree::<Person, EdgeType1>(2);
        assert_eq!(matches, vec![PersonId::new(1)]);
        let matches = nd.find_entities_by_degree::<Person, EdgeType1>(1);
        assert_eq!(matches, vec![PersonId::new(2), PersonId::new(3)]);
    }
}

#[cfg(test)]
#[allow(clippy::float_cmp)]
// Tests for the API.
mod test_api {
    use crate::context::Context;
    use crate::network::edge::Edge;
    use crate::network::ContextNetworkExt;
    use crate::prelude::*;
    use crate::random::ContextRandomExt;
    use crate::{define_edge_type, define_entity, define_property, define_rng};

    define_entity!(Person);

    define_edge_type!(struct EdgeType1(pub u32), Person);
    define_property!(struct Age(u8), Person);

    fn setup() -> (Context, PersonId, PersonId) {
        let mut context = Context::new();
        let person1 = context.add_entity((Age(1),)).unwrap();
        let person2 = context.add_entity((Age(2),)).unwrap();

        (context, person1, person2)
    }

    #[test]
    fn add_edge() {
        let (mut context, person1, person2) = setup();

        context
            .add_edge::<Person, EdgeType1>(person1, person2, 0.01, EdgeType1(1))
            .unwrap();
        assert_eq!(
            context
                .get_edge::<Person, EdgeType1>(person1, person2)
                .unwrap()
                .weight,
            0.01
        );
        assert_eq!(
            context.get_edges::<Person, EdgeType1>(person1),
            vec![Edge {
                neighbor: person2,
                weight: 0.01,
                inner: EdgeType1(1)
            }]
        );
    }

    #[test]
    fn remove_edge() {
        let (mut context, person1, person2) = setup();
        assert!(context
            .remove_edge::<Person, EdgeType1>(person1, person2)
            .is_none());
        context
            .add_edge::<Person, EdgeType1>(person1, person2, 0.01, EdgeType1(1))
            .unwrap();
        assert!(context
            .remove_edge::<Person, EdgeType1>(person1, person2)
            .is_some());
        assert!(context
            .get_edge::<Person, EdgeType1>(person1, person2)
            .is_none());
        assert_eq!(context.get_edges::<Person, EdgeType1>(person1).len(), 0);
    }

    #[test]
    fn add_edge_bidi() {
        let (mut context, person1, person2) = setup();

        context
            .add_edge_bidi::<Person, EdgeType1>(person1, person2, 0.01, EdgeType1(1))
            .unwrap();
        assert_eq!(
            context
                .get_edge::<Person, EdgeType1>(person1, person2)
                .unwrap()
                .weight,
            0.01
        );
        assert_eq!(
            context
                .get_edge::<Person, EdgeType1>(person2, person1)
                .unwrap()
                .weight,
            0.01
        );
    }

    #[test]
    fn add_edge_different_weights() {
        let (mut context, person1, person2) = setup();

        context
            .add_edge::<Person, EdgeType1>(person1, person2, 0.01, EdgeType1(1))
            .unwrap();
        context
            .add_edge::<Person, EdgeType1>(person2, person1, 0.02, EdgeType1(1))
            .unwrap();
        assert_eq!(
            context
                .get_edge::<Person, EdgeType1>(person1, person2)
                .unwrap()
                .weight,
            0.01
        );
        assert_eq!(
            context
                .get_edge::<Person, EdgeType1>(person2, person1)
                .unwrap()
                .weight,
            0.02
        );
    }

    #[test]
    fn get_matching_edges_weight() {
        let (mut context, person1, person2) = setup();
        let person3 = context.add_entity((Age(3),)).unwrap();

        context
            .add_edge::<Person, EdgeType1>(person1, person2, 0.01, EdgeType1(1))
            .unwrap();
        context
            .add_edge::<Person, EdgeType1>(person1, person3, 0.03, EdgeType1(1))
            .unwrap();
        let edges = context
            .get_matching_edges::<Person, EdgeType1>(person1, |_context, edge| edge.weight > 0.01);
        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0].neighbor, person3);
    }

    #[test]
    fn get_matching_edges_inner() {
        let (mut context, person1, person2) = setup();
        let person3 = context.add_entity((Age(3),)).unwrap();

        context
            .add_edge::<Person, EdgeType1>(person1, person2, 0.01, EdgeType1(1))
            .unwrap();
        context
            .add_edge::<Person, EdgeType1>(person1, person3, 0.03, EdgeType1(3))
            .unwrap();
        let edges = context
            .get_matching_edges::<Person, EdgeType1>(person1, |_context, edge| edge.inner.0 == 3);
        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0].neighbor, person3);
    }

    #[test]
    fn get_matching_edges_person_property() {
        let (mut context, person1, person2) = setup();
        let person3 = context.add_entity((Age(3),)).unwrap();

        context
            .add_edge::<Person, EdgeType1>(person1, person2, 0.01, EdgeType1(1))
            .unwrap();
        context
            .add_edge::<Person, EdgeType1>(person1, person3, 0.03, EdgeType1(3))
            .unwrap();
        let edges = context.get_matching_edges::<Person, EdgeType1>(person1, |context, edge| {
            context.match_entity(edge.neighbor, (Age(3),))
        });
        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0].neighbor, person3);
    }

    #[test]
    fn select_random_edge() {
        define_rng!(NetworkTestRng);

        let (mut context, person1, person2) = setup();
        let person3 = context.add_entity((Age(3),)).unwrap();
        context.init_random(42);

        context
            .add_edge::<Person, EdgeType1>(person1, person2, 0.01, EdgeType1(1))
            .unwrap();
        context
            .add_edge::<Person, EdgeType1>(person1, person3, 10_000_000.0, EdgeType1(3))
            .unwrap();

        let edge = context
            .select_random_edge::<Person, EdgeType1, _>(NetworkTestRng, person1)
            .unwrap();
        assert_eq!(edge.neighbor, person3);
        assert_eq!(edge.inner, EdgeType1(3));
    }
}

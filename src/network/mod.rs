//! Contact networks.
//!
//! A network is a set of directed edges over one entity type, split into layers.
//! [`define_network!`](crate::define_network) declares one: an enum whose variants are the
//! layers (households, schools, ..) or a unit struct for a single layer, with an optional
//! per-edge data type. Each entity has at most one edge per neighbor per layer, and no
//! edge to itself. Edges are read as borrowed slices and sampled in place.
//!
//! ```
//! use ixa::prelude::*;
//!
//! define_entity!(Person);
//! define_network!(Contact, Person, layers = { Household, School }, data = f64);
//!
//! let mut context = Context::new();
//! let p1 = context.add_entity(with!(Person)).unwrap();
//! let p2 = context.add_entity(with!(Person)).unwrap();
//! context.add_edge_bidi(Contact::Household, p1, p2, 1.0).unwrap();
//! context.add_edge(Contact::School, p1, p2, 0.5).unwrap();
//!
//! let mut total_rate = 0.0;
//! for &layer in Contact::ALL {
//!     for edge in context.get_edges(layer, p1) {
//!         total_rate += edge.data;
//!     }
//! }
//! assert_eq!(total_rate, 1.5);
//!
//! context.clear_layer(Contact::School);
//! assert_eq!(context.get_degree(Contact::School, p1), 0);
//! assert_eq!(context.get_degree(Contact::Household, p2), 1);
//! ```

pub mod edge;
mod network_store;
mod registry;
mod storage;

use std::any::Any;
use std::cell::OnceCell;

pub use edge::Edge;
use network_store::{typed, typed_mut, NetworkStore};
use rand::Rng;
pub use registry::Network;
#[doc(hidden)]
pub use registry::{get_registered_network_count, initialize_network_id, register_network};
use storage::LayerStorage;

use crate::context::Context;
use crate::entity::entity_store::get_registered_entity_count;
use crate::entity::{ContextEntitiesExt, Entity, EntityId};
use crate::error::IxaError;
use crate::random::{ContextRandomExt, RngId};
use crate::{define_data_plugin, ContextBase};

/// The two directions removed by `remove_edge_bidi`: `(a -> b, b -> a)`.
type EdgePair<N> = (
    Edge<<N as Network>::Entity, <N as Network>::Data>,
    Edge<<N as Network>::Entity, <N as Network>::Data>,
);

pub(crate) struct NetworkData {
    /// A `NetworkStore<E>` per `Entity::id()`.
    network_stores: Vec<OnceCell<Box<dyn Any>>>,
}

// The rules are enforced here; `ContextNetworkExt` documents them.
impl NetworkData {
    fn new() -> Self {
        let network_stores = (0..get_registered_entity_count())
            .map(|_| OnceCell::new())
            .collect();
        NetworkData { network_stores }
    }

    fn store<E: Entity>(&self) -> &NetworkStore<E> {
        let cell = self.network_stores.get(E::id()).unwrap_or_else(|| {
            panic!(
                "Ixa internal error: NetworkStore for Entity {} not found",
                E::name()
            )
        });
        typed(cell, NetworkStore::<E>::new_boxed, E::name())
    }

    fn store_mut<E: Entity>(&mut self) -> &mut NetworkStore<E> {
        let cell = self.network_stores.get_mut(E::id()).unwrap_or_else(|| {
            panic!(
                "Ixa internal error: NetworkStore for Entity {} not found",
                E::name()
            )
        });
        typed_mut(cell, NetworkStore::<E>::new_boxed, E::name())
    }

    fn layer<N: Network>(&self, layer: N) -> &LayerStorage<N::Entity, N::Data> {
        self.store::<N::Entity>().get::<N>().layer(layer)
    }

    fn layer_mut<N: Network>(&mut self, layer: N) -> &mut LayerStorage<N::Entity, N::Data> {
        self.store_mut::<N::Entity>()
            .get_mut::<N>()
            .layer_mut(layer)
    }

    fn add_edge<N: Network>(
        &mut self,
        layer: N,
        from: EntityId<N::Entity>,
        to: EntityId<N::Entity>,
        data: N::Data,
    ) -> Result<(), IxaError> {
        if from == to {
            return Err(IxaError::CannotMakeEdgeToSelf);
        }
        self.layer_mut(layer)
            .add_edge(from, Edge { neighbor: to, data })
    }

    fn add_edge_bidi<N: Network>(
        &mut self,
        layer: N,
        a: EntityId<N::Entity>,
        b: EntityId<N::Entity>,
        data: N::Data,
    ) -> Result<(), IxaError> {
        if a == b {
            return Err(IxaError::CannotMakeEdgeToSelf);
        }
        let storage = self.layer_mut(layer);
        if storage.position(a, b).is_some() || storage.position(b, a).is_some() {
            return Err(IxaError::EdgeAlreadyExists);
        }
        storage.push_unchecked(
            a,
            Edge {
                neighbor: b,
                data: data.clone(),
            },
        );
        storage.push_unchecked(b, Edge { neighbor: a, data });
        Ok(())
    }

    fn remove_edge<N: Network>(
        &mut self,
        layer: N,
        from: EntityId<N::Entity>,
        to: EntityId<N::Entity>,
    ) -> Option<Edge<N::Entity, N::Data>> {
        self.layer_mut(layer).remove_edge(from, to)
    }

    fn remove_edge_bidi<N: Network>(
        &mut self,
        layer: N,
        a: EntityId<N::Entity>,
        b: EntityId<N::Entity>,
    ) -> Option<EdgePair<N>> {
        let storage = self.layer_mut(layer);
        let forward = storage.position(a, b)?;
        let backward = storage.position(b, a)?;
        // `a != b` (no self-loops), so removing from one list leaves the other position valid.
        Some((
            storage.remove_at(a, forward),
            storage.remove_at(b, backward),
        ))
    }

    fn update_edge<N: Network>(
        &mut self,
        layer: N,
        from: EntityId<N::Entity>,
        to: EntityId<N::Entity>,
        f: impl FnOnce(&mut N::Data),
    ) -> bool {
        match self.layer_mut(layer).get_edge_mut(from, to) {
            Some(edge) => {
                f(&mut edge.data);
                true
            }
            None => false,
        }
    }

    fn clear_layer<N: Network>(&mut self, layer: N) {
        self.layer_mut(layer).clear();
    }

    fn get_edge<N: Network>(
        &self,
        layer: N,
        from: EntityId<N::Entity>,
        to: EntityId<N::Entity>,
    ) -> Option<&Edge<N::Entity, N::Data>> {
        self.layer(layer).get_edge(from, to)
    }

    fn get_edges<N: Network>(
        &self,
        layer: N,
        entity: EntityId<N::Entity>,
    ) -> &[Edge<N::Entity, N::Data>] {
        self.layer(layer).get_edges(entity)
    }

    fn find_entities_by_degree<N: Network>(
        &self,
        layer: N,
        degree: usize,
        population: usize,
    ) -> Vec<EntityId<N::Entity>> {
        self.layer(layer)
            .find_entities_by_degree(degree, population)
    }

    fn degree_distribution<N: Network>(&self, layer: N, population: usize) -> Vec<usize> {
        self.layer(layer).degree_distribution(population)
    }
}

define_data_plugin!(NetworkPlugin, NetworkData, NetworkData::new());

// Public API.
pub trait ContextNetworkExt: ContextBase + ContextRandomExt + ContextEntitiesExt {
    /// Adds an edge in `layer` from `from` to `to` carrying `data`. The data is stored as
    /// given: a numeric rate is not checked here, only when it is used as a sampling weight.
    ///
    /// # Errors
    /// [`IxaError::CannotMakeEdgeToSelf`] if `from == to`; [`IxaError::EdgeAlreadyExists`]
    /// if `from` already has an edge to `to` in this layer.
    fn add_edge<N: Network>(
        &mut self,
        layer: N,
        from: EntityId<N::Entity>,
        to: EntityId<N::Entity>,
        data: N::Data,
    ) -> Result<(), IxaError> {
        self.get_data_mut(NetworkPlugin)
            .add_edge(layer, from, to, data)
    }

    /// Adds an edge in each direction between `a` and `b`, both carrying `data`, or
    /// neither if either direction would fail.
    ///
    /// # Errors
    /// As [`add_edge`](Self::add_edge), checked for both directions first.
    fn add_edge_bidi<N: Network>(
        &mut self,
        layer: N,
        a: EntityId<N::Entity>,
        b: EntityId<N::Entity>,
        data: N::Data,
    ) -> Result<(), IxaError> {
        self.get_data_mut(NetworkPlugin)
            .add_edge_bidi(layer, a, b, data)
    }

    /// Adds every `(from, to, data)` edge in `layer`, stopping at the first failure and
    /// leaving the edges added before it in place.
    ///
    /// # Errors
    /// As [`add_edge`](Self::add_edge).
    fn add_edges<N: Network>(
        &mut self,
        layer: N,
        edges: impl IntoIterator<Item = (EntityId<N::Entity>, EntityId<N::Entity>, N::Data)>,
    ) -> Result<(), IxaError> {
        let network = self.get_data_mut(NetworkPlugin);
        for (from, to, data) in edges {
            network.add_edge(layer, from, to, data)?;
        }
        Ok(())
    }

    /// Removes and returns the edge in `layer` from `from` to `to`, or `None` if there is
    /// none. The order of the remaining edges of `from` is not preserved.
    #[must_use = "returns None if there was no edge to remove"]
    fn remove_edge<N: Network>(
        &mut self,
        layer: N,
        from: EntityId<N::Entity>,
        to: EntityId<N::Entity>,
    ) -> Option<Edge<N::Entity, N::Data>> {
        self.get_data_mut(NetworkPlugin)
            .remove_edge(layer, from, to)
    }

    /// Removes the edges in both directions between `a` and `b` and returns them as
    /// `(a -> b, b -> a)`, or removes nothing and returns `None` if either is missing.
    #[must_use = "returns None if the edges were not both present"]
    fn remove_edge_bidi<N: Network>(
        &mut self,
        layer: N,
        a: EntityId<N::Entity>,
        b: EntityId<N::Entity>,
    ) -> Option<EdgePair<N>> {
        self.get_data_mut(NetworkPlugin)
            .remove_edge_bidi(layer, a, b)
    }

    /// Changes the data of the edge in `layer` from `from` to `to` in place, keeping its
    /// position among the edges of `from`. Returns `false`, without calling `f`, if there
    /// is no such edge.
    #[must_use = "returns false if there is no such edge"]
    fn update_edge<N: Network>(
        &mut self,
        layer: N,
        from: EntityId<N::Entity>,
        to: EntityId<N::Entity>,
        f: impl FnOnce(&mut N::Data),
    ) -> bool {
        self.get_data_mut(NetworkPlugin)
            .update_edge(layer, from, to, f)
    }

    /// Removes every edge in `layer`.
    fn clear_layer<N: Network>(&mut self, layer: N) {
        self.get_data_mut(NetworkPlugin).clear_layer(layer);
    }

    /// The edge in `layer` from `from` to `to`, if any.
    #[must_use]
    fn get_edge<N: Network>(
        &self,
        layer: N,
        from: EntityId<N::Entity>,
        to: EntityId<N::Entity>,
    ) -> Option<&Edge<N::Entity, N::Data>> {
        self.get_data(NetworkPlugin).get_edge(layer, from, to)
    }

    /// The outgoing edges of `entity` in `layer`, in insertion order unless an edge was
    /// removed. Empty if it has none.
    #[must_use]
    fn get_edges<N: Network>(
        &self,
        layer: N,
        entity: EntityId<N::Entity>,
    ) -> &[Edge<N::Entity, N::Data>] {
        self.get_data(NetworkPlugin).get_edges(layer, entity)
    }

    /// The number of outgoing edges of `entity` in `layer`.
    #[must_use]
    fn get_degree<N: Network>(&self, layer: N, entity: EntityId<N::Entity>) -> usize {
        self.get_edges(layer, entity).len()
    }

    /// Every entity with exactly `degree` outgoing edges in `layer`, in id order.
    #[must_use]
    fn find_entities_by_degree<N: Network>(
        &self,
        layer: N,
        degree: usize,
    ) -> Vec<EntityId<N::Entity>> {
        let population = self.get_entity_count::<N::Entity>();
        self.get_data(NetworkPlugin)
            .find_entities_by_degree(layer, degree, population)
    }

    /// The out-degree distribution of `layer` over the whole population: `result[d]` is
    /// the number of entities with exactly `d` outgoing edges, so the result has
    /// `max degree + 1` entries and sums to the entity count.
    #[must_use]
    fn degree_distribution<N: Network>(&self, layer: N) -> Vec<usize> {
        let population = self.get_entity_count::<N::Entity>();
        self.get_data(NetworkPlugin)
            .degree_distribution(layer, population)
    }

    /// A uniformly random outgoing edge of `entity` in `layer`, or `None` if it has none.
    #[must_use]
    fn sample_edge<N: Network, R: RngId + 'static>(
        &self,
        rng_id: R,
        layer: N,
        entity: EntityId<N::Entity>,
    ) -> Option<&Edge<N::Entity, N::Data>>
    where
        R::RngType: Rng,
    {
        let edges = self.get_edges(layer, entity);
        if edges.is_empty() {
            return None;
        }
        let index = self.sample_range(rng_id, 0..edges.len());
        Some(&edges[index])
    }

    /// An outgoing edge of `entity` in `layer` drawn with probability proportional to
    /// `weight(edge)`, or `None` if it has none. `weight` is called twice per edge and
    /// nothing is allocated; store a weight in the edge data if it is expensive to compute.
    ///
    /// # Panics
    /// If a weight is negative or not finite, the weights overflow when summed, or no
    /// weight is positive.
    #[must_use]
    fn sample_edge_weighted<N: Network, R: RngId + 'static>(
        &self,
        rng_id: R,
        layer: N,
        entity: EntityId<N::Entity>,
        weight: impl Fn(&Edge<N::Entity, N::Data>) -> f64,
    ) -> Option<&Edge<N::Entity, N::Data>>
    where
        R::RngType: Rng,
    {
        let edges = self.get_edges(layer, entity);
        if edges.is_empty() {
            return None;
        }
        let mut total = 0.0;
        for edge in edges {
            let w = weight(edge);
            assert!(
                w >= 0.0 && w.is_finite(),
                "edge weights must be nonnegative finite numbers (edge to {:?})",
                edge.neighbor
            );
            total += w;
        }
        assert!(total > 0.0, "at least one edge weight must be positive");
        assert!(
            total.is_finite(),
            "edge weights must not overflow when summed"
        );

        let draw: f64 = self.sample_range(rng_id, 0.0..total);
        let mut acc = 0.0;
        let mut last_positive = None;
        for edge in edges {
            let w = weight(edge);
            if w > 0.0 {
                last_positive = Some(edge);
                acc += w;
                if draw < acc {
                    return Some(edge);
                }
            }
        }
        // Rounding can leave `draw` past the running sum; the last positive edge is the
        // one it belongs to.
        last_positive
    }
}

impl ContextNetworkExt for Context {}

#[cfg(test)]
// Tests for the inner core.
mod test_inner {
    use super::NetworkData;
    use crate::error::IxaError;
    use crate::network::edge::Edge;
    use crate::{define_entity, define_network};

    define_entity!(Person);

    define_network!(Weighted, Person, data = f32);
    define_network!(Plain, Person);
    define_network!(Flagged, Person, data = bool);
    define_network!(Layered, Person, layers = { Home, School }, data = f32);

    #[test]
    fn add_edge_with_data() {
        let mut nd = NetworkData::new();
        nd.add_edge(Flagged, PersonId::new(1), PersonId::new(2), true)
            .unwrap();
        let edge = nd
            .get_edge(Flagged, PersonId::new(1), PersonId::new(2))
            .unwrap();
        assert!(edge.data);
    }

    #[test]
    fn add_two_edges() {
        let mut nd = NetworkData::new();
        nd.add_edge(Weighted, PersonId::new(1), PersonId::new(2), 0.01)
            .unwrap();
        nd.add_edge(Weighted, PersonId::new(1), PersonId::new(3), 0.02)
            .unwrap();
        assert_eq!(
            nd.get_edges(Weighted, PersonId::new(1)),
            &[
                Edge {
                    neighbor: PersonId::new(2),
                    data: 0.01
                },
                Edge {
                    neighbor: PersonId::new(3),
                    data: 0.02
                }
            ]
        );
    }

    #[test]
    fn layers_are_independent() {
        let mut nd = NetworkData::new();
        nd.add_edge(Layered::Home, PersonId::new(1), PersonId::new(2), 0.01)
            .unwrap();
        nd.add_edge(Layered::School, PersonId::new(1), PersonId::new(2), 0.02)
            .unwrap();
        assert_eq!(
            nd.get_edge(Layered::Home, PersonId::new(1), PersonId::new(2))
                .unwrap()
                .data,
            0.01
        );
        assert_eq!(
            nd.get_edge(Layered::School, PersonId::new(1), PersonId::new(2))
                .unwrap()
                .data,
            0.02
        );
        assert_eq!(nd.get_edges(Layered::Home, PersonId::new(1)).len(), 1);

        nd.clear_layer(Layered::School);
        assert!(nd.get_edges(Layered::School, PersonId::new(1)).is_empty());
        assert_eq!(nd.get_edges(Layered::Home, PersonId::new(1)).len(), 1);
    }

    #[test]
    fn clear_keeps_degree_zero_reporting() {
        let mut nd = NetworkData::new();
        nd.add_edge(Layered::School, PersonId::new(1), PersonId::new(2), 1.0)
            .unwrap();
        nd.clear_layer(Layered::School);
        assert_eq!(
            nd.find_entities_by_degree(Layered::School, 0, 3),
            vec![PersonId::new(0), PersonId::new(1), PersonId::new(2)]
        );
    }

    #[test]
    fn networks_are_independent() {
        let mut nd = NetworkData::new();
        nd.add_edge(Weighted, PersonId::new(1), PersonId::new(2), 0.01)
            .unwrap();
        nd.add_edge(Flagged, PersonId::new(1), PersonId::new(2), false)
            .unwrap();
        assert_eq!(nd.get_edges(Weighted, PersonId::new(1)).len(), 1);
        assert_eq!(nd.get_edges(Flagged, PersonId::new(1)).len(), 1);
        assert!(nd.get_edges(Plain, PersonId::new(1)).is_empty());
    }

    #[test]
    fn add_edge_twice_fails() {
        let mut nd = NetworkData::new();
        nd.add_edge(Weighted, PersonId::new(1), PersonId::new(2), 0.01)
            .unwrap();
        assert!(matches!(
            nd.add_edge(Weighted, PersonId::new(1), PersonId::new(2), 0.02),
            Err(IxaError::EdgeAlreadyExists)
        ));
    }

    #[test]
    fn add_remove_add_edge() {
        let mut nd = NetworkData::new();
        nd.add_edge(Weighted, PersonId::new(1), PersonId::new(2), 0.01)
            .unwrap();
        let removed = nd
            .remove_edge(Weighted, PersonId::new(1), PersonId::new(2))
            .unwrap();
        assert_eq!(removed.data, 0.01);
        assert!(nd
            .get_edge(Weighted, PersonId::new(1), PersonId::new(2))
            .is_none());
        nd.add_edge(Weighted, PersonId::new(1), PersonId::new(2), 0.02)
            .unwrap();
        assert_eq!(
            nd.get_edge(Weighted, PersonId::new(1), PersonId::new(2))
                .unwrap()
                .data,
            0.02
        );
    }

    #[test]
    fn remove_nonexistent_edge() {
        let mut nd = NetworkData::new();
        assert!(nd
            .remove_edge(Weighted, PersonId::new(1), PersonId::new(2))
            .is_none());
    }

    #[test]
    fn add_edge_to_self() {
        let mut nd = NetworkData::new();
        assert!(matches!(
            nd.add_edge(Weighted, PersonId::new(1), PersonId::new(1), 0.01),
            Err(IxaError::CannotMakeEdgeToSelf)
        ));
    }

    #[test]
    fn find_people_by_degree() {
        let mut nd = NetworkData::new();
        nd.add_edge(Weighted, PersonId::new(1), PersonId::new(2), 0.0)
            .unwrap();
        nd.add_edge(Weighted, PersonId::new(1), PersonId::new(3), 0.0)
            .unwrap();
        nd.add_edge(Weighted, PersonId::new(2), PersonId::new(3), 0.0)
            .unwrap();
        nd.add_edge(Weighted, PersonId::new(3), PersonId::new(2), 0.0)
            .unwrap();
        assert_eq!(
            nd.find_entities_by_degree(Weighted, 2, 4),
            vec![PersonId::new(1)]
        );
        assert_eq!(
            nd.find_entities_by_degree(Weighted, 1, 4),
            vec![PersonId::new(2), PersonId::new(3)]
        );
        assert_eq!(
            nd.find_entities_by_degree(Weighted, 0, 4),
            vec![PersonId::new(0)]
        );
    }
}

#[cfg(test)]
// Tests for the API.
mod test_api {
    use crate::context::Context;
    use crate::network::{ContextNetworkExt, Network};
    use crate::prelude::*;
    use crate::random::ContextRandomExt;
    use crate::{define_entity, define_network, define_property, define_rng, with};

    define_entity!(Person);
    define_property!(struct Age(u8), Person);

    define_network!(Contact, Person, layers = { Household, School }, data = f64);
    define_network!(Tagged, Person, data = u32);
    define_rng!(NetworkTestRng);

    fn setup() -> (Context, PersonId, PersonId) {
        let mut context = Context::new();
        let person1 = context.add_entity(with!(Person, Age(1))).unwrap();
        let person2 = context.add_entity(with!(Person, Age(2))).unwrap();
        (context, person1, person2)
    }

    #[test]
    fn layer_metadata() {
        assert_eq!(Contact::ALL, &[Contact::Household, Contact::School]);
        assert_eq!(Contact::School.layer(), 1);
        assert_eq!(Tagged::ALL, &[Tagged]);
        assert_eq!(Tagged.layer(), 0);
        assert_eq!(<Contact as Network>::name(), "Contact");
    }

    #[test]
    fn add_edges_bulk() {
        let (mut context, person1, person2) = setup();
        let person3 = context.add_entity(with!(Person, Age(3))).unwrap();
        context
            .add_edges(
                Contact::School,
                [(person1, person2, 1.0), (person1, person3, 2.0)],
            )
            .unwrap();
        assert_eq!(context.get_degree(Contact::School, person1), 2);
        assert_eq!(context.get_degree(Contact::Household, person1), 0);

        // A duplicate stops the batch and reports the error.
        let result = context.add_edges(
            Contact::School,
            [(person2, person1, 1.0), (person1, person2, 1.0)],
        );
        assert!(matches!(result, Err(IxaError::EdgeAlreadyExists)));
        assert_eq!(context.get_degree(Contact::School, person2), 1);
    }

    #[test]
    fn add_edge_bidi() {
        let (mut context, person1, person2) = setup();
        context
            .add_edge_bidi(Contact::Household, person1, person2, 0.5)
            .unwrap();
        assert_eq!(
            context
                .get_edge(Contact::Household, person1, person2)
                .unwrap()
                .data,
            0.5
        );
        assert_eq!(
            context
                .get_edge(Contact::Household, person2, person1)
                .unwrap()
                .data,
            0.5
        );

        // Either direction existing fails the whole call and inserts nothing.
        let person3 = context.add_entity(with!(Person, Age(3))).unwrap();
        context
            .add_edge(Contact::Household, person3, person1, 1.0)
            .unwrap();
        assert!(matches!(
            context.add_edge_bidi(Contact::Household, person1, person3, 1.0),
            Err(IxaError::EdgeAlreadyExists)
        ));
        assert!(context
            .get_edge(Contact::Household, person1, person3)
            .is_none());
        assert!(matches!(
            context.add_edge_bidi(Contact::Household, person1, person1, 1.0),
            Err(IxaError::CannotMakeEdgeToSelf)
        ));
    }

    // The repeated layer is the point of the test; the generated `match` has a dead arm.
    #[allow(unreachable_patterns)]
    mod repeated {
        use super::Person;

        #[derive(Copy, Clone, Debug, PartialEq, Eq)]
        pub enum Repeated {
            A,
            B,
        }
        crate::impl_network!(
            Repeated,
            Person,
            layers = [Repeated::A, Repeated::A, Repeated::B]
        );
    }
    use repeated::Repeated;

    #[test]
    #[should_panic(expected = "every layer must appear once in ALL")]
    fn repeated_layer_panics() {
        let (mut context, person1, person2) = setup();
        let _ = context.add_edge(Repeated::B, person1, person2, ());
    }

    #[test]
    fn remove_edge_bidi() {
        let (mut context, person1, person2) = setup();
        let person3 = context.add_entity(with!(Person, Age(3))).unwrap();
        context
            .add_edge_bidi(Contact::Household, person1, person2, 0.5)
            .unwrap();
        context
            .add_edge_bidi(Contact::School, person1, person2, 0.25)
            .unwrap();

        let (forward, backward) = context
            .remove_edge_bidi(Contact::Household, person1, person2)
            .unwrap();
        assert_eq!((forward.neighbor, forward.data), (person2, 0.5));
        assert_eq!((backward.neighbor, backward.data), (person1, 0.5));
        assert_eq!(context.get_degree(Contact::Household, person1), 0);
        assert_eq!(context.get_degree(Contact::Household, person2), 0);
        assert_eq!(context.get_degree(Contact::School, person1), 1);

        // Only one direction present: nothing is removed.
        context
            .add_edge(Contact::Household, person1, person3, 1.0)
            .unwrap();
        assert!(context
            .remove_edge_bidi(Contact::Household, person1, person3)
            .is_none());
        assert!(context
            .remove_edge_bidi(Contact::Household, person3, person1)
            .is_none());
        assert!(context
            .get_edge(Contact::Household, person1, person3)
            .is_some());
    }

    #[test]
    fn update_edge_in_place() {
        let (mut context, person1, person2) = setup();
        let person3 = context.add_entity(with!(Person, Age(3))).unwrap();
        context.add_edge(Tagged, person1, person2, 1).unwrap();
        context.add_edge(Tagged, person1, person3, 2).unwrap();

        assert!(context.update_edge(Tagged, person1, person2, |data| *data += 10));
        let edges: Vec<_> = context
            .get_edges(Tagged, person1)
            .iter()
            .map(|e| (e.neighbor, e.data))
            .collect();
        assert_eq!(edges, vec![(person2, 11), (person3, 2)]);

        let mut called = false;
        assert!(!context.update_edge(Tagged, person2, person1, |_| called = true));
        assert!(!called);
        assert!(context.get_edge(Tagged, person2, person1).is_none());
    }

    #[test]
    #[should_panic(expected = "must not overflow")]
    fn sample_edge_weighted_overflow_panics() {
        let (mut context, person1, person2) = setup();
        let person3 = context.add_entity(with!(Person, Age(3))).unwrap();
        context.init_random(42);
        context
            .add_edge(Contact::Household, person1, person2, f64::MAX)
            .unwrap();
        context
            .add_edge(Contact::Household, person1, person3, f64::MAX)
            .unwrap();
        let _ =
            context.sample_edge_weighted(NetworkTestRng, Contact::Household, person1, |e| e.data);
    }

    #[test]
    fn find_isolates_over_population() {
        let (mut context, person1, person2) = setup();
        let person3 = context.add_entity(with!(Person, Age(3))).unwrap();
        context
            .add_edge(Contact::School, person1, person2, 1.0)
            .unwrap();
        assert_eq!(
            context.find_entities_by_degree(Contact::School, 0),
            vec![person2, person3]
        );
        assert_eq!(
            context.find_entities_by_degree(Contact::Household, 0),
            vec![person1, person2, person3]
        );
    }

    #[test]
    fn degree_distribution() {
        let (mut context, person1, person2) = setup();
        let person3 = context.add_entity(with!(Person, Age(3))).unwrap();
        assert_eq!(context.degree_distribution(Contact::School), vec![3]);

        context
            .add_edge(Contact::School, person1, person2, 1.0)
            .unwrap();
        context
            .add_edge(Contact::School, person1, person3, 1.0)
            .unwrap();
        context
            .add_edge(Contact::School, person2, person3, 1.0)
            .unwrap();
        assert_eq!(context.degree_distribution(Contact::School), vec![1, 1, 1]);
        assert_eq!(context.degree_distribution(Contact::Household), vec![3]);
    }

    #[test]
    fn filter_edges_by_neighbor_property() {
        let (mut context, person1, person2) = setup();
        let person3 = context.add_entity(with!(Person, Age(3))).unwrap();
        context.add_edge(Tagged, person1, person2, 1).unwrap();
        context.add_edge(Tagged, person1, person3, 3).unwrap();

        let old: Vec<_> = context
            .get_edges(Tagged, person1)
            .iter()
            .filter(|edge| context.match_entity(edge.neighbor, with!(Person, Age(3))))
            .collect();
        assert_eq!(old.len(), 1);
        assert_eq!(old[0].neighbor, person3);
        assert_eq!(old[0].data, 3);
    }

    #[test]
    fn sample_edge_weighted() {
        let (mut context, person1, person2) = setup();
        let person3 = context.add_entity(with!(Person, Age(3))).unwrap();
        context.init_random(42);

        assert!(context
            .sample_edge_weighted(NetworkTestRng, Contact::Household, person1, |e| e.data)
            .is_none());

        context
            .add_edge(Contact::Household, person1, person2, 0.0)
            .unwrap();
        context
            .add_edge(Contact::Household, person1, person3, 10.0)
            .unwrap();
        for _ in 0..20 {
            let edge = context
                .sample_edge_weighted(NetworkTestRng, Contact::Household, person1, |e| e.data)
                .unwrap();
            assert_eq!(edge.neighbor, person3);
        }
    }

    #[test]
    fn sample_edge_uniform() {
        let (mut context, person1, person2) = setup();
        let person3 = context.add_entity(with!(Person, Age(3))).unwrap();
        context.init_random(42);

        assert!(context
            .sample_edge(NetworkTestRng, Tagged, person1)
            .is_none());
        context.add_edge(Tagged, person1, person2, 1).unwrap();
        context.add_edge(Tagged, person1, person3, 2).unwrap();

        let mut seen = [false; 2];
        for _ in 0..50 {
            let edge = context
                .sample_edge(NetworkTestRng, Tagged, person1)
                .unwrap();
            seen[edge.data as usize - 1] = true;
        }
        assert_eq!(seen, [true, true]);
    }

    #[test]
    #[should_panic(expected = "at least one edge weight must be positive")]
    fn sample_edge_weighted_all_zero_panics() {
        let (mut context, person1, person2) = setup();
        context.init_random(42);
        context
            .add_edge(Contact::Household, person1, person2, 0.0)
            .unwrap();
        let _ =
            context.sample_edge_weighted(NetworkTestRng, Contact::Household, person1, |e| e.data);
    }
}

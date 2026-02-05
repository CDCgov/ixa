/*!

A `Network<E: Entity, ET: EdgeType<E>>` is the concrete type implementing the storage of adjacency lists. Each `EdgeType<E>` has its own `Network`.

An adjacency list is just a list of `Edge<E: Entity, ET: EdgeType<E>>`s. Thus, `Network` is just a list of
lists of edges. This data structure shares the storage optimization behavior of
`PropertyValueStoreCore<E: Entity, P: Property<E>>` in that it is indexed by `EntityId<E>` and grows as needed. The
difference is that it stores adjacency lists (edges) rather that properties and doesn't manage an index.

This structure is only concerned with storage of unique edges. Other business logic is the responsibility
of higher level API. A `NetworkStore<E: Entity>` holds all of the networks for a given `Entity` type.

*/

use std::any::Any;

use crate::entity::{Entity, EntityId};
use crate::network::edge::{Edge, EdgeType};
use crate::IxaError;

/// The underlying storage type representing the adjacency list
pub(super) type AdjacencyList<E, ET> = Vec<Edge<E, ET>>;
/// The underlying storage type storing adjacency lists
pub(super) type AdjacencyListVec<E, ET> = Vec<AdjacencyList<E, ET>>;

pub(super) struct Network<E: Entity, ET: EdgeType<E>> {
    /// The backing storage vector for the adjacency lists.
    pub(super) adjacency_lists: AdjacencyListVec<E, ET>,
}

impl<E: Entity, ET: EdgeType<E>> Default for Network<E, ET> {
    fn default() -> Self {
        Self {
            adjacency_lists: AdjacencyListVec::default(),
        }
    }
}

#[allow(unused)]
impl<E: Entity, ET: EdgeType<E> + 'static> Network<E, ET> {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn new_boxed() -> Box<dyn Any> {
        Box::new(Self::new())
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            adjacency_lists: AdjacencyListVec::with_capacity(capacity),
        }
    }

    /// Ensures capacity for at least `additional` more elements
    pub fn reserve(&mut self, additional: usize) {
        self.adjacency_lists.reserve(additional);
    }

    /// Returns a clone of the adjacency list for the given entity. Returns an empty list
    /// if the list is empty.
    pub fn get_list_cloned(&self, entity_id: EntityId<E>) -> AdjacencyList<E, ET> {
        self.adjacency_lists
            .get(entity_id.0)
            .cloned()
            .unwrap_or_default()
    }

    /// Returns an immutable reference to the adjacency list for the given entity,
    /// or `None` if the list is empty.
    pub fn get_list(&self, entity_id: EntityId<E>) -> Option<&AdjacencyList<E, ET>> {
        self.adjacency_lists.get(entity_id.0)
    }

    /// Returns a mutable reference to the adjacency list for the given entity,
    /// or `None` if the list is empty.
    pub fn get_list_mut(&mut self, entity_id: EntityId<E>) -> Option<&mut AdjacencyList<E, ET>> {
        self.adjacency_lists.get_mut(entity_id.0)
    }

    /// Inserts the given edge into the adjacency list for the given entity.
    /// If an edge having the same neighbor as the given edge exists, an error is returned.
    pub fn add_edge(&mut self, entity_id: EntityId<E>, edge: Edge<E, ET>) -> Result<(), IxaError> {
        let index = entity_id.0;

        // Ensure the adjacency list exists
        if index >= self.adjacency_lists.len() {
            self.adjacency_lists
                .resize_with(index + 1, Default::default);
        }

        let edges = &mut self.adjacency_lists[index];

        // Enforce uniqueness by neighbor
        if edges.iter().any(|e| e.neighbor == edge.neighbor) {
            return Err(IxaError::IxaError("Edge already exists".into()));
        }

        edges.push(edge);
        Ok(())
    }

    /// Remove the edge from the given entity to the given neighbor and return it, or
    /// `None` if the edge does not exist.
    pub fn remove_edge(
        &mut self,
        entity_id: EntityId<E>,
        neighbor: EntityId<E>,
    ) -> Option<Edge<E, ET>> {
        self.adjacency_lists.get_mut(entity_id.0).and_then(|edges| {
            edges
                .iter()
                .position(|edge| edge.neighbor == neighbor)
                .map(|pos| edges.swap_remove(pos))
        })
    }

    /// Returns an immutable reference to the edge from the given entity
    /// to the given neighbor, or `None` if the edge does not exist.
    pub fn get_edge(&self, entity_id: EntityId<E>, neighbor: EntityId<E>) -> Option<&Edge<E, ET>> {
        let index = entity_id.0;
        self.adjacency_lists
            .get(index)
            .and_then(|edges| edges.iter().find(|edge| edge.neighbor == neighbor))
    }

    /// Returns a mutable reference to the edge from the given entity
    /// to the given neighbor, or `None` if the edge does not exist.
    pub fn get_edge_mut(
        &mut self,
        entity_id: EntityId<E>,
        neighbor: EntityId<E>,
    ) -> Option<&mut Edge<E, ET>> {
        let index = entity_id.0;
        self.adjacency_lists
            .get_mut(index)
            .and_then(|edges| edges.iter_mut().find(|edge| edge.neighbor == neighbor))
    }

    /// Returns a list of `EntityId<E>`s having exactly the given number of neighbors.
    pub fn find_entities_by_degree(&self, degree: usize) -> Vec<EntityId<E>> {
        self.adjacency_lists
            .iter()
            .enumerate()
            .filter_map(|(pos, edges)| (edges.len() == degree).then_some(EntityId::new(pos)))
            .collect()
    }
}

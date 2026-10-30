/*!

`LayerStorage<E, D>` holds the adjacency lists of one layer: a `Vec<Edge<E, D>>` per
entity, indexed by `EntityId<E>` and grown as needed, like a property column. It
enforces one edge per (source, neighbor) pair and nothing else; the rest of the rules
live in the public API.

*/

use crate::entity::{Entity, EntityId};
use crate::network::edge::Edge;
use crate::IxaError;

pub(super) struct LayerStorage<E: Entity, D> {
    lists: Vec<Vec<Edge<E, D>>>,
}

impl<E: Entity, D> Default for LayerStorage<E, D> {
    fn default() -> Self {
        Self { lists: Vec::new() }
    }
}

impl<E: Entity, D> LayerStorage<E, D> {
    /// Inserts `edge` into the list of `from`, or fails if `from` already has an edge
    /// to the same neighbor.
    pub fn add_edge(&mut self, from: EntityId<E>, edge: Edge<E, D>) -> Result<(), IxaError> {
        if self.get_edge(from, edge.neighbor).is_some() {
            return Err(IxaError::EdgeAlreadyExists);
        }
        self.push_unchecked(from, edge);
        Ok(())
    }

    /// Appends `edge` to the list of `from` without the uniqueness check. The caller
    /// must have checked.
    pub fn push_unchecked(&mut self, from: EntityId<E>, edge: Edge<E, D>) {
        let index = from.0;
        if index >= self.lists.len() {
            self.lists.resize_with(index + 1, Vec::new);
        }
        self.lists[index].push(edge);
    }

    /// Removes and returns the edge from `from` to `to`, if any. The order of the
    /// remaining edges of `from` is not preserved.
    pub fn remove_edge(&mut self, from: EntityId<E>, to: EntityId<E>) -> Option<Edge<E, D>> {
        self.lists.get_mut(from.0).and_then(|edges| {
            edges
                .iter()
                .position(|edge| edge.neighbor == to)
                .map(|pos| edges.swap_remove(pos))
        })
    }

    pub fn get_edge(&self, from: EntityId<E>, to: EntityId<E>) -> Option<&Edge<E, D>> {
        self.get_edges(from).iter().find(|edge| edge.neighbor == to)
    }

    /// The outgoing edges of `from`, empty if it has none.
    pub fn get_edges(&self, from: EntityId<E>) -> &[Edge<E, D>] {
        self.lists.get(from.0).map_or(&[], Vec::as_slice)
    }

    /// Entities with exactly `degree` outgoing edges. Entities past the end of the
    /// storage have degree zero but are not reported.
    pub fn find_entities_by_degree(&self, degree: usize) -> Vec<EntityId<E>> {
        self.lists
            .iter()
            .enumerate()
            .filter_map(|(pos, edges)| (edges.len() == degree).then_some(EntityId::new(pos)))
            .collect()
    }

    /// Removes every edge. Each entity's list keeps its allocation, so re-adding the
    /// edges later (reopening a closed setting) does not reallocate.
    pub fn clear(&mut self) {
        for list in &mut self.lists {
            list.clear();
        }
    }
}

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
        if self.position(from, edge.neighbor).is_some() {
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
        let pos = self.position(from, to)?;
        Some(self.remove_at(from, pos))
    }

    /// The index of the edge from `from` to `to` in the list of `from`, if any.
    pub fn position(&self, from: EntityId<E>, to: EntityId<E>) -> Option<usize> {
        self.get_edges(from)
            .iter()
            .position(|edge| edge.neighbor == to)
    }

    /// Swap-removes the edge at `pos`, a position from [`position`](Self::position).
    pub fn remove_at(&mut self, from: EntityId<E>, pos: usize) -> Edge<E, D> {
        self.lists[from.0].swap_remove(pos)
    }

    pub fn get_edge(&self, from: EntityId<E>, to: EntityId<E>) -> Option<&Edge<E, D>> {
        self.get_edges(from).iter().find(|edge| edge.neighbor == to)
    }

    pub fn get_edge_mut(&mut self, from: EntityId<E>, to: EntityId<E>) -> Option<&mut Edge<E, D>> {
        self.lists
            .get_mut(from.0)?
            .iter_mut()
            .find(|edge| edge.neighbor == to)
    }

    /// The outgoing edges of `from`, empty if it has none.
    pub fn get_edges(&self, from: EntityId<E>) -> &[Edge<E, D>] {
        self.lists.get(from.0).map_or(&[], Vec::as_slice)
    }

    /// Entities among ids `0..population` with exactly `degree` outgoing edges.
    pub fn find_entities_by_degree(&self, degree: usize, population: usize) -> Vec<EntityId<E>> {
        (0..population)
            .map(EntityId::new)
            .filter(|&id| self.get_edges(id).len() == degree)
            .collect()
    }

    /// Counts of entities by out-degree over ids `0..population`: `result[d]` is the
    /// number of entities with degree `d`, so the result has `max degree + 1` entries.
    pub fn degree_distribution(&self, population: usize) -> Vec<usize> {
        let mut distribution = Vec::new();
        for id in 0..population {
            let degree = self.get_edges(EntityId::new(id)).len();
            if degree >= distribution.len() {
                distribution.resize(degree + 1, 0);
            }
            distribution[degree] += 1;
        }
        distribution
    }

    /// Removes every edge. Each entity's list keeps its allocation, so re-adding the
    /// edges later (reopening a closed setting) does not reallocate.
    pub fn clear(&mut self) {
        for list in &mut self.lists {
            list.clear();
        }
    }
}

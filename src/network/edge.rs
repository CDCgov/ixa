use std::fmt::Debug;

use crate::entity::{Entity, EntityId};

/// A directed edge stored in the adjacency list of its source entity, so the
/// source is implicit.
pub struct Edge<E: Entity, D> {
    /// The entity this edge points to.
    pub neighbor: EntityId<E>,
    /// The per-edge data declared by the network's `data = ..` option.
    pub data: D,
}

// The generics prevent the compiler from "seeing" that `Edge` satisfies these
// traits if they are derived, so they are implemented by hand.
impl<E: Entity, D: Clone> Clone for Edge<E, D> {
    fn clone(&self) -> Self {
        Self {
            neighbor: self.neighbor,
            data: self.data.clone(),
        }
    }
}

impl<E: Entity, D: Copy> Copy for Edge<E, D> {}

impl<E: Entity, D: Debug> Debug for Edge<E, D> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Edge")
            .field("neighbor", &self.neighbor)
            .field("data", &self.data)
            .finish()
    }
}

impl<E: Entity, D: PartialEq> PartialEq for Edge<E, D> {
    fn eq(&self, other: &Self) -> bool {
        self.neighbor == other.neighbor && self.data == other.data
    }
}

/*!

`LayerView<N>` is the public read-only handle on one layer's `LayerStorage`.

*/

use super::edge::Edge;
use super::registry::Network;
use super::storage::LayerStorage;
use crate::entity::EntityId;

/// A read-only view of one layer of a network, from
/// [`get_layer`](crate::network::ContextNetworkExt::get_layer). The layer is found once,
/// so reading many entities through a view is faster than the same calls on the context.
pub struct LayerView<'a, N: Network> {
    storage: &'a LayerStorage<N::Entity, N::Data>,
}

// Derives would require `N: Clone` and `N: Copy` bounds the view does not need.
impl<N: Network> Clone for LayerView<'_, N> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<N: Network> Copy for LayerView<'_, N> {}

impl<'a, N: Network> LayerView<'a, N> {
    pub(super) fn new(storage: &'a LayerStorage<N::Entity, N::Data>) -> Self {
        Self { storage }
    }

    /// The number of edges in the layer, counting each direction.
    #[must_use]
    pub fn edge_count(&self) -> usize {
        self.storage.edge_count()
    }

    /// Every edge in the layer with its source, by source id and then in the order of
    /// [`get_edges`](Self::get_edges).
    #[allow(clippy::type_complexity)]
    pub fn iter(
        &self,
    ) -> impl Iterator<Item = (EntityId<N::Entity>, &'a Edge<N::Entity, N::Data>)> {
        self.storage.iter()
    }

    /// The outgoing edges of `entity`, empty if it has none.
    #[must_use]
    pub fn get_edges(&self, entity: EntityId<N::Entity>) -> &'a [Edge<N::Entity, N::Data>] {
        self.storage.get_edges(entity)
    }

    /// The edge from `from` to `to`, if any.
    #[must_use]
    pub fn get_edge(
        &self,
        from: EntityId<N::Entity>,
        to: EntityId<N::Entity>,
    ) -> Option<&'a Edge<N::Entity, N::Data>> {
        self.storage.get_edge(from, to)
    }

    /// The number of outgoing edges of `entity`.
    #[must_use]
    pub fn get_degree(&self, entity: EntityId<N::Entity>) -> usize {
        self.storage.get_edges(entity).len()
    }
}

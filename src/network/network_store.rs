/*!

A `NetworkStorage<N>` is one `LayerStorage` per layer of the network `N`, found by
`N::layer()`. It is the data container of `NetworkPlugin<N>`, a data plugin per network
type, so a layer is one plugin lookup and one index from the context.

*/

use std::marker::PhantomData;

use super::registry::Network;
use super::storage::LayerStorage;
use crate::{DataPlugin, PluginContext};

/// The data plugin holding the edges of the network `N`. Its index is `N::id()`.
pub(super) struct NetworkPlugin<N>(PhantomData<N>);

impl<N> NetworkPlugin<N> {
    pub const KEY: Self = Self(PhantomData);
}

impl<N: Network> DataPlugin for NetworkPlugin<N> {
    type DataContainer = NetworkStorage<N>;

    fn init<C: PluginContext>(_context: &C) -> Self::DataContainer {
        NetworkStorage::new()
    }

    fn index_within_context() -> usize {
        N::id()
    }
}

pub(super) struct NetworkStorage<N: Network> {
    layers: Vec<LayerStorage<N::Entity, N::Data>>,
}

impl<N: Network> NetworkStorage<N> {
    pub fn new() -> Self {
        // Catches a hand-written `impl Network` whose `layer()` disagrees with `ALL`, which
        // would otherwise make loops over `ALL` visit a layer twice. (The macros check this
        // at compile time.)
        for (position, &layer) in N::ALL.iter().enumerate() {
            assert_eq!(
                layer.layer(),
                position,
                "network {}: every layer must appear once in ALL, in layer() order, but {layer:?} \
                 is at position {position}",
                N::name(),
            );
        }
        let layers = N::ALL.iter().map(|_| LayerStorage::default()).collect();
        Self { layers }
    }

    pub fn layer(&self, layer: N) -> &LayerStorage<N::Entity, N::Data> {
        let index = layer.layer();
        self.layers
            .get(index)
            .unwrap_or_else(|| Self::layer_out_of_range(layer, index))
    }

    pub fn layer_mut(&mut self, layer: N) -> &mut LayerStorage<N::Entity, N::Data> {
        let index = layer.layer();
        self.layers
            .get_mut(index)
            .unwrap_or_else(|| Self::layer_out_of_range(layer, index))
    }

    // Only a hand-written `impl Network` whose `layer()` disagrees with `ALL` gets here;
    // `layers.len() == ALL.len()` by construction.
    fn layer_out_of_range(layer: N, index: usize) -> ! {
        panic!(
            "network {}: layer {layer:?} has index {index} but ALL has {} layers",
            N::name(),
            N::ALL.len()
        )
    }
}

/*!

A `NetworkStore<E>` holds a `NetworkStorage<N>` for every network `N` over the entity
type `E`, found by `N::id()`. A `NetworkStorage<N>` is one `LayerStorage` per layer of
`N`, found by `N::layer()`.

*/

use std::any::Any;
use std::cell::OnceCell;
use std::marker::PhantomData;

use super::registry::{get_registered_network_count, Network};
use super::storage::LayerStorage;
use crate::entity::Entity;

pub(super) struct NetworkStorage<N: Network> {
    layers: Vec<LayerStorage<N::Entity, N::Data>>,
}

impl<N: Network> NetworkStorage<N> {
    fn new_boxed() -> Box<dyn Any> {
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
        Box::new(Self { layers })
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

pub(super) struct NetworkStore<E: Entity> {
    networks: Vec<OnceCell<Box<dyn Any>>>,
    _phantom: PhantomData<E>,
}

impl<E: Entity> NetworkStore<E> {
    pub fn new_boxed() -> Box<dyn Any> {
        let networks = (0..get_registered_network_count::<E>())
            .map(|_| OnceCell::new())
            .collect();
        Box::new(Self {
            networks,
            _phantom: PhantomData,
        })
    }

    pub fn get<N: Network<Entity = E>>(&self) -> &NetworkStorage<N> {
        typed_slot(
            &self.networks,
            N::id(),
            NetworkStorage::<N>::new_boxed,
            N::name(),
        )
    }

    pub fn get_mut<N: Network<Entity = E>>(&mut self) -> &mut NetworkStorage<N> {
        typed_slot_mut(
            &mut self.networks,
            N::id(),
            NetworkStorage::<N>::new_boxed,
            N::name(),
        )
    }
}

fn not_registered(name: &str) -> String {
    format!(
        "{name} was not registered before its storage was created. Networks declared with \
         `define_network!` or `impl_network!` register at startup; a hand-written `impl \
         Network` must call `register_network::<N>()` before the first network access."
    )
}

/// The slot at `index` of a registry-sized list of lazily built storages, initialized with
/// `init` if empty and returned as a `T`. `name` is the network or entity type for panics.
pub(super) fn typed_slot<'a, T: Any>(
    slots: &'a [OnceCell<Box<dyn Any>>],
    index: usize,
    init: fn() -> Box<dyn Any>,
    name: &str,
) -> &'a T {
    slots
        .get(index)
        .unwrap_or_else(|| panic!("{}", not_registered(name)))
        .get_or_init(init)
        .downcast_ref()
        .unwrap_or_else(|| panic!("Ixa internal error: wrong storage type for {name}"))
}

/// Mutable twin of [`typed_slot`].
pub(super) fn typed_slot_mut<'a, T: Any>(
    slots: &'a mut [OnceCell<Box<dyn Any>>],
    index: usize,
    init: fn() -> Box<dyn Any>,
    name: &str,
) -> &'a mut T {
    let cell = slots
        .get_mut(index)
        .unwrap_or_else(|| panic!("{}", not_registered(name)));
    if cell.get().is_none() {
        cell.set(init()).unwrap();
    }
    cell.get_mut()
        .unwrap()
        .downcast_mut()
        .unwrap_or_else(|| panic!("Ixa internal error: wrong storage type for {name}"))
}

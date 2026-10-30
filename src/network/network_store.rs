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
        let layers = N::ALL.iter().map(|_| LayerStorage::default()).collect();
        Box::new(Self { layers })
    }

    pub fn layer(&self, layer: N) -> &LayerStorage<N::Entity, N::Data> {
        &self.layers[layer.layer()]
    }

    pub fn layer_mut(&mut self, layer: N) -> &mut LayerStorage<N::Entity, N::Data> {
        &mut self.layers[layer.layer()]
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
        let cell = self
            .networks
            .get(N::id())
            .unwrap_or_else(|| panic!("Ixa internal error: network {} not registered", N::name()));
        typed(cell, NetworkStorage::<N>::new_boxed, N::name())
    }

    pub fn get_mut<N: Network<Entity = E>>(&mut self) -> &mut NetworkStorage<N> {
        let cell = self
            .networks
            .get_mut(N::id())
            .unwrap_or_else(|| panic!("Ixa internal error: network {} not registered", N::name()));
        typed_mut(cell, NetworkStorage::<N>::new_boxed, N::name())
    }
}

/// Initializes `cell` with `init` if empty and returns its contents as a `T`.
pub(super) fn typed<'a, T: Any>(
    cell: &'a OnceCell<Box<dyn Any>>,
    init: fn() -> Box<dyn Any>,
    name: &str,
) -> &'a T {
    cell.get_or_init(init)
        .downcast_ref()
        .unwrap_or_else(|| panic!("Ixa internal error: wrong storage type for {name}"))
}

/// Mutable twin of [`typed`].
pub(super) fn typed_mut<'a, T: Any>(
    cell: &'a mut OnceCell<Box<dyn Any>>,
    init: fn() -> Box<dyn Any>,
    name: &str,
) -> &'a mut T {
    if cell.get().is_none() {
        cell.set(init()).unwrap();
    }
    cell.get_mut()
        .unwrap()
        .downcast_mut()
        .unwrap_or_else(|| panic!("Ixa internal error: wrong storage type for {name}"))
}

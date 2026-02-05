/*!

A `NetworkStore<E: Entity>` holds all `Network<E: Entity, ET: EdgeType<E>>`s for a given `Entity` type.

`NetworkStore` uses the fact that `EdgeType<E>` uses the registry pattern to efficiently
access the `Network` for a given `EdgeType`.

*/

use std::any::Any;
use std::cell::OnceCell;
use std::marker::PhantomData;

use super::edge::{get_registered_edge_type_count, EdgeType};
use super::network::Network;
use crate::entity::Entity;

pub(super) struct NetworkStore<E: Entity> {
    networks: Vec<OnceCell<Box<dyn Any>>>,
    _phantom: PhantomData<E>,
}

impl<E: Entity> NetworkStore<E> {
    #[must_use]
    pub fn new() -> Self {
        let edge_type_count = get_registered_edge_type_count::<E>();
        let networks = (0..edge_type_count)
            .map(|_| OnceCell::new())
            .collect::<Vec<_>>();

        Self {
            networks,
            _phantom: PhantomData,
        }
    }

    #[must_use]
    pub fn new_boxed() -> Box<dyn Any> {
        Box::new(Self::new())
    }

    /// Returns an immutable reference to the `Network<E, ET>`.
    #[must_use]
    pub fn get<ET: EdgeType<E>>(&self) -> &Network<E, ET> {
        self.networks
            .get(ET::id())
            .unwrap_or_else(|| {
                panic!(
                    "internal error: Network for EdgeType {} not found",
                    ET::name()
                )
            })
            .get_or_init(Network::<E, ET>::new_boxed)
            .downcast_ref()
            .unwrap_or_else(|| {
                panic!(
                    "internal error: found wrong Network type when accessing EdgeType {}",
                    ET::name()
                )
            })
    }

    /// Returns a mutable reference to the `Network<E, ET>`.
    #[must_use]
    pub fn get_mut<ET: EdgeType<E>>(&mut self) -> &mut Network<E, ET> {
        let cell = self.networks.get_mut(ET::id()).unwrap_or_else(|| {
            panic!(
                "internal error: Network for EdgeType {} not found",
                ET::name()
            )
        });

        // Lazily initialize if needed.
        if cell.get().is_none() {
            cell.set(Network::<E, ET>::new_boxed()).unwrap();
        }

        // Now the `unwrap` on `get_mut` is guaranteed to succeed.
        cell.get_mut().unwrap().downcast_mut().unwrap_or_else(|| {
            panic!(
                "internal error: found wrong Network type when accessing EdgeType {}",
                ET::name()
            )
        })
    }
}

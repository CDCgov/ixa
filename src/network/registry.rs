/*!

The [`Network`] trait, and the registration of each network type as a data plugin of
its own.

*/

use std::fmt::Debug;
use std::marker::PhantomData;
use std::sync::atomic::AtomicUsize;

use super::network_store::NetworkPlugin;
use crate::entity::Entity;
use crate::{add_data_plugin_to_registry, initialize_data_plugin_index, IxaError};

/// A network over one entity type, made of one or more layers. Implemented by
/// [`define_network!`](crate::define_network) and [`impl_network!`](crate::impl_network).
///
/// A value of the implementing type names a layer. A layered network is an enum with
/// one variant per layer; a single-layer network is a unit struct.
pub trait Network: Copy + Eq + Debug + 'static {
    /// The entity type the edges connect.
    type Entity: Entity;
    /// Data stored on every edge. `()` when the network has none.
    type Data: Clone + Debug + PartialEq + 'static;

    /// Every layer, in index order.
    const ALL: &'static [Self];

    /// The position of this layer in [`ALL`](Self::ALL).
    fn layer(self) -> usize;

    /// Checks data before it is stored on an edge, by `add_edge`, `add_edge_bidi`,
    /// `add_edges`, `update_edge` and `add_or_update_edge`. The network macros implement it
    /// from their `validate = ..` option, or without one require `f64` and `f32` data to be
    /// finite and non-negative and accept any other type as is. This default, used only by
    /// a hand-written `impl Network`, accepts everything.
    ///
    /// # Errors
    /// [`IxaError::InvalidEdgeData`](crate::IxaError::InvalidEdgeData) if the data is rejected.
    fn validate(data: &Self::Data) -> Result<(), IxaError> {
        let _ = data;
        Ok(())
    }

    #[must_use]
    fn name() -> &'static str {
        let full = std::any::type_name::<Self>();
        // Splitting any string, including an empty one, always yields at least one segment.
        full.rsplit("::").next().unwrap()
    }

    /// The index of this network's storage among the data plugins of a context. The
    /// macros assign it from a `ctor`; a hand-written impl must call
    /// [`register_network`](crate::network::register_network) before the first context
    /// is created.
    #[must_use]
    fn id() -> usize;
}

/// Registers `N` by forcing its id to be assigned. Called from the `ctor` emitted by
/// `impl_network!`.
pub fn register_network<N: Network>() {
    let _ = N::id();
}

/// Registers the data plugin that holds the edges of `N` and claims its index into
/// `network_id`, unless another thread already did, in which case the existing index is
/// returned. Registering and claiming together means an index is never claimed for a
/// network the context does not know about.
#[must_use]
pub fn initialize_network_id<N: Network>(network_id: &AtomicUsize) -> usize {
    add_data_plugin_to_registry::<NetworkPlugin<N>>();
    initialize_data_plugin_index(network_id)
}

/// Autoref-specialization probe for the default edge data check; resolves only at a
/// concrete type, so it is used by the network macros and never in generic code.
#[doc(hidden)]
pub struct EdgeDataProbe<D>(pub PhantomData<D>);

#[doc(hidden)]
pub trait FloatEdgeData<D> {
    fn is_valid_edge_data(&self, data: &D) -> bool;
}

impl FloatEdgeData<f64> for EdgeDataProbe<f64> {
    fn is_valid_edge_data(&self, data: &f64) -> bool {
        data.is_finite() && *data >= 0.0
    }
}

impl FloatEdgeData<f32> for EdgeDataProbe<f32> {
    fn is_valid_edge_data(&self, data: &f32) -> bool {
        data.is_finite() && *data >= 0.0
    }
}

#[doc(hidden)]
pub trait AnyEdgeData<D> {
    fn is_valid_edge_data(&self, data: &D) -> bool;
}

impl<D> AnyEdgeData<D> for &EdgeDataProbe<D> {
    fn is_valid_edge_data(&self, _data: &D) -> bool {
        true
    }
}

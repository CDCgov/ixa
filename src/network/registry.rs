/*!

The [`Network`] trait and the registry that assigns each network type a dense
index per entity type, so a `NetworkStore<E>` can be sized when it is created.

*/

use std::fmt::Debug;
use std::marker::PhantomData;
use std::sync::atomic::AtomicUsize;
use std::sync::{LazyLock, Mutex};

use crate::data_plugin::claim_index;
use crate::entity::Entity;
use crate::{HashMap, HashMapExt, IxaError};

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
    /// `add_edges` and `update_edge`. The network macros implement it from their
    /// `validate = ..` option, or without one require `f64` and `f32` data to be finite and
    /// non-negative and accept any other type as is. This default, used only by a
    /// hand-written `impl Network`, accepts everything.
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

    /// The index of this network among the networks over `Self::Entity`, assigned on
    /// first access and registered from a `ctor` so the count is known before any
    /// `NetworkStore<E>` is built. A hand-written impl must call
    /// [`register_network`](crate::network::register_network) before the first network
    /// access instead.
    #[must_use]
    fn id() -> usize;
}

/// Next network id per `Entity::id()`. Each entity type has its own series.
static NEXT_NETWORK_ID_BY_ENTITY: LazyLock<Mutex<HashMap<usize, usize>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Returns the number of registered networks over the entity type `E`.
#[must_use]
pub fn get_registered_network_count<E: Entity>() -> usize {
    let map = NEXT_NETWORK_ID_BY_ENTITY.lock().unwrap();
    *map.get(&E::id()).unwrap_or(&0)
}

/// Registers `N` by forcing its id to be assigned. Called from the `ctor` emitted by
/// `impl_network!`.
pub fn register_network<N: Network>() {
    let _ = N::id();
}

/// Assigns the next id for networks over `E` to `network_id` unless another thread
/// already initialized it, in which case the existing id is returned and the counter is
/// left alone.
#[must_use]
pub fn initialize_network_id<E: Entity>(network_id: &AtomicUsize) -> usize {
    let mut guard = NEXT_NETWORK_ID_BY_ENTITY.lock().unwrap();
    let next = guard.entry(E::id()).or_insert(0);
    claim_index(network_id, next)
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

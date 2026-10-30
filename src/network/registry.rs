/*!

The [`Network`] trait and the registry that assigns each network type a dense
index per entity type, so a `NetworkStore<E>` can be sized when it is created.

*/

use std::fmt::Debug;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{LazyLock, Mutex};

use crate::entity::Entity;
use crate::{HashMap, HashMapExt};

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

    #[must_use]
    fn name() -> &'static str {
        let full = std::any::type_name::<Self>();
        // Splitting any string, including an empty one, always yields at least one segment.
        full.rsplit("::").next().unwrap()
    }

    /// The index of this network among the networks over `Self::Entity`, assigned on
    /// first access and registered from a `ctor` so the count is known before any
    /// `NetworkStore<E>` is built.
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
/// left alone. Same protocol as `initialize_data_plugin_index`; for the ordering argument
/// see <https://github.com/CDCgov/ixa/pull/477#discussion_r2244302872>.
#[must_use]
pub fn initialize_network_id<E: Entity>(network_id: &AtomicUsize) -> usize {
    let mut guard = NEXT_NETWORK_ID_BY_ENTITY.lock().unwrap();
    let candidate = guard.entry(E::id()).or_insert_with(|| 0);

    match network_id.compare_exchange(usize::MAX, *candidate, Ordering::AcqRel, Ordering::Acquire) {
        Ok(_) => {
            *candidate += 1;
            *candidate - 1
        }
        Err(existing) => existing,
    }
}

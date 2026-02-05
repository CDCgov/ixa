use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{LazyLock, Mutex};

use crate::entity::{Entity, EntityId};

#[derive(Copy, Debug, PartialEq)]
/// An edge in the network graph. Edges are directed, so the
/// source person is implicit.
pub struct Edge<E: Entity, ET: EdgeType<E>> {
    /// The person this edge points to.
    pub neighbor: EntityId<E>,
    /// The weight associated with the edge.
    pub weight: f32,
    /// An inner value defined by type `T`. Often a ZST.
    pub inner: ET,
}

// Generics prevent the compiler from "seeing" that `Edge` always satisfies these
// traits if they are derived.
impl<E: Entity, ET: EdgeType<E>> Clone for Edge<E, ET> {
    fn clone(&self) -> Self {
        Self {
            neighbor: self.neighbor,
            weight: self.weight,
            inner: self.inner.clone(),
        }
    }
}

pub trait EdgeType<E: Entity>: Clone + 'static {
    fn name() -> &'static str {
        let full = std::any::type_name::<Self>();
        full.rsplit("::").next().unwrap()
    }

    /// The index of this item in the owner, which is initialized globally per type
    /// upon first access. We explicitly initialize this in a `ctor` in order to know
    /// how many [`EdgeType<E>`] types exist globally when we construct any `NetworkStore<E>`.
    fn id() -> usize;
}

/// A map from Entity ID to a count of the edge types already associated with the entity. The value for the key is
/// equivalent to the next edge type ID that will be assigned to the next edge type that requests an ID. Each `Entity`
/// type has its own series of increasing edge type IDs.
static NEXT_EDGE_TYPE_ID_BY_ENTITY: LazyLock<Mutex<HashMap<usize, usize>>> =
    LazyLock::new(|| Mutex::new(HashMap::default()));

/// Returns the number of registered edge types for the entity type `E`.
pub fn get_registered_edge_type_count<E: Entity>() -> usize {
    let map = NEXT_EDGE_TYPE_ID_BY_ENTITY.lock().unwrap();
    *map.get(&E::id()).unwrap_or(&0)
}

/// Adds a new edge type to the registry. The job of this method is to create whatever
/// "singleton" data/metadata is associated with the [`EdgeType`] if it doesn't already
/// exist, which in this case is only the value of `EdgeType::id()`.
pub fn add_to_edge_type_to_registry<E: Entity, ET: EdgeType<E>>() {
    let _ = ET::id();
}

/// Encapsulates the synchronization logic for initializing an [`EdgeType<E>`]'s ID.
///
/// Acquires a global lock on the next available edge type ID for the given entity type `E`,
/// but only increments it if we successfully initialize the provided ID. The ID of an
/// edge type is
/// assigned at runtime but only once per type. It's possible for a single
/// type to attempt to initialize its index multiple times from different threads,
/// which is why all this synchronization is required. However, the overhead
/// is negligible, as this initialization only happens once upon first access.
///
/// In fact, for our use case we know we are calling this function
/// once for each type in each `EdgeType<E>`'s `ctor` function, which
/// should be the only time this method is ever called for the type.
pub fn initialize_edge_type_id<E: Entity>(edge_type_id: &AtomicUsize) -> usize {
    // Acquire a global lock.
    let mut guard = NEXT_EDGE_TYPE_ID_BY_ENTITY.lock().unwrap();
    let candidate = guard.entry(E::id()).or_insert_with(|| 0);

    // Try to claim the candidate index. Here we guard against the potential race condition that
    // another instance of this plugin in another thread just initialized the index prior to us
    // obtaining the lock. If the index has been initialized beneath us, we do not update
    // NEXT_EDGE_TYPE_ID_BY_ENTITY, we just return the value `edge_type_id` was initialized to.
    // For a justification of the data ordering, see:
    //     https://github.com/CDCgov/ixa/pull/477#discussion_r2244302872
    match edge_type_id.compare_exchange(usize::MAX, *candidate, Ordering::AcqRel, Ordering::Acquire)
    {
        Ok(_) => {
            // We won the race — increment the global next edge-type ID and return the new ID.
            *candidate += 1;
            *candidate - 1
        }
        Err(existing) => {
            // Another thread beat us — don’t increment the global next edge-type ID,
            // just return the existing one.
            existing
        }
    }
}

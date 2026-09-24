/*!

The entity-erased interface to a concrete
[`PropertyStoreCore`](crate::entity::property_store_core::PropertyStoreCore).

Typed entity and property operations should downcast once to the concrete core and remain on the
typed side of this boundary. This trait is for operations that genuinely do not know the entity
type, including whole-store traversal.

*/

use std::any::Any;

/// The entity-erased interface implemented by every concrete property store.
pub trait PropertyStore: Any {
    /// Allocates the next entity ID and reports whether entity-created events have subscribers.
    fn allocate_entity_id(&mut self) -> (usize, bool);

    /// Returns the number of entity instances owned by this store.
    fn entity_count(&self) -> usize;
}

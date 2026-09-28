/*!

The entity-erased interface to a concrete
[`PropertyStoreCore`](crate::entity::property_store_core::PropertyStoreCore).

Typed entity and property operations should downcast once to the concrete core and remain on the
typed side of this boundary. This trait is for operations that genuinely do not know the entity
type, including whole-store traversal.

*/

use std::any::Any;
use std::io::{Read, Write};

use super::entity_store::EntityManifest;
use crate::IxaError;

/// The entity-erased interface implemented by every concrete property store.
pub(crate) trait PropertyStore: Any {
    /// Allocates the next entity ID and reports whether entity-created events have subscribers.
    fn allocate_entity_id(&mut self) -> (usize, bool);

    /// Returns the number of entity instances owned by this store.
    fn entity_count(&self) -> usize;

    /// Used to resolve serialized property data to the concrete `PropertyValueStoreCore<E, P>`
    /// it should be deserialized into.
    fn entity_type_name(&self) -> &'static str;

    /// Creates a manifest entry for this entity for population persistence.
    fn population_manifest(&self) -> EntityManifest;

    /// Write out all persistable properties for this entity.
    fn encode_properties(
        &self,
        writer: &mut bincode_next::IoWriter<'_, &mut dyn Write>,
    ) -> Result<(), IxaError>;

    /// Restore all persisted properties for this entity.
    fn decode_properties(
        &mut self,
        manifest: &EntityManifest,
        reader: &mut dyn Read,
    ) -> Result<(), IxaError>;
}

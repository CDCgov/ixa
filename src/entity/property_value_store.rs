/*!

The `PropertyValueStore` trait is the type-erased interface to property value storage.

Responsibilities:

- type-erased interface to the index
- Create "partial" property change events during property value updates

*/

use std::any::Any;
use std::io::{Read, Write};

use crate::entity::events::{
    PartialPropertyChangeEventBox, PartialPropertyChangeEventCore, PropertyChangeEvent,
};
use crate::entity::index::{IndexCountResult, IndexSetResult};
use crate::entity::property::{Property, PropertyInitializationKind};
use crate::entity::property_value_store_core::PropertyValueStoreCore;
use crate::entity::{Entity, EntityId};
use crate::{Context, IxaError};

/// The `PropertyValueStore` trait defines the type-erased interface to the concrete property value storage.
pub(crate) trait PropertyValueStore<E: Entity>: Any {
    /// Used to resolve serialized property data to the concrete `PropertyValueStoreCore<E, P>`
    /// it should be deserialized into.
    fn property_type_name(&self) -> &'static str;

    /// Used to filter which properties are serialized and validate deserialized data.
    fn is_derived(&self) -> bool;

    /// Persists the storage.
    fn encode(
        &self,
        entity_count: usize,
        writer: &mut bincode_next::IoWriter<'_, &mut dyn Write>,
    ) -> Result<(), IxaError>;

    /// Restores persisted storage.
    fn decode(&mut self, entity_count: usize, reader: &mut dyn Read) -> Result<(), IxaError>;

    // Methods related to updating a value of a dependency
    /// Fetches the existing value of the property for the given `entity_id` and returns a
    /// `PartialPropertyChangeEvent` object wrapping the previous value and `entity_id`.
    fn create_partial_property_change(
        &self,
        // The entity_id has been type-erased but is guaranteed by the caller to be an `EntityId<E>`.
        entity_id: EntityId<E>,
        context: &Context,
    ) -> PartialPropertyChangeEventBox;

    /// Returns whether a property write needs the partial change-event machinery.
    ///
    /// This is true if the property has change-event subscribers, value change counters, or an index.
    fn should_create_partial_change(&self, context: &Context) -> bool;

    // Index-related methods. Anything beyond these requires the `PropertyValueStoreCore<E, P>`.

    fn get_index_set_for_query_parts(&self, parts: &[&dyn Any]) -> IndexSetResult<'_, E>;

    fn get_index_count_for_query_parts(&self, parts: &[&dyn Any]) -> IndexCountResult;
}

impl<E: Entity, P: Property<E>> PropertyValueStore<E> for PropertyValueStoreCore<E, P> {
    fn property_type_name(&self) -> &'static str {
        std::any::type_name::<P>()
    }

    fn is_derived(&self) -> bool {
        P::is_derived()
    }

    fn encode(
        &self,
        entity_count: usize,
        writer: &mut bincode_next::IoWriter<'_, &mut dyn Write>,
    ) -> Result<(), IxaError> {
        debug_assert!(!P::is_derived());
        validate_length::<E, P>(entity_count, self.data.len())?;

        bincode_next::serde::encode_into_writer(
            &self.data,
            writer,
            bincode_next::config::standard(),
        )
        .map_err(|source| IxaError::PopulationEncodeError {
            item: format!(
                "property {} for entity {}",
                std::any::type_name::<P>(),
                std::any::type_name::<E>()
            ),
            source,
        })
    }

    fn decode(&mut self, entity_count: usize, reader: &mut dyn Read) -> Result<(), IxaError> {
        debug_assert!(!P::is_derived());
        debug_assert!(self.data.is_empty());
        debug_assert!(self.index.is_none());
        debug_assert!(self.value_change_counters.is_empty());

        let mut reader = reader;
        let data: Vec<P> = bincode_next::serde::decode_from_std_read(
            &mut reader,
            bincode_next::config::standard(),
        )
        .map_err(|source| IxaError::PopulationDecodeError {
            item: format!(
                "property {} for entity {}",
                std::any::type_name::<P>(),
                std::any::type_name::<E>()
            ),
            source,
        })?;
        validate_length::<E, P>(entity_count, data.len())?;
        self.data = data;
        Ok(())
    }

    fn create_partial_property_change(
        &self,
        entity_id: EntityId<E>,
        context: &Context,
    ) -> PartialPropertyChangeEventBox {
        // Compute the existing value of the property for the given `entity_id` and return a
        // `PartialPropertyChangeEvent` object wrapping the previous value and `entity_id`.

        let previous_value = if P::is_derived() {
            P::compute_derived(context, entity_id)
        } else {
            self.get(entity_id)
        };

        smallbox::smallbox!(PartialPropertyChangeEventCore::<E, P>::new(
            entity_id,
            previous_value,
        ))
    }

    fn should_create_partial_change(&self, context: &Context) -> bool {
        context.has_event_handlers::<PropertyChangeEvent<E, P>>()
            || !self.value_change_counters.is_empty()
            || self.index.is_some()
    }

    fn get_index_set_for_query_parts(&self, parts: &[&dyn Any]) -> IndexSetResult<'_, E> {
        match P::value_from_query_parts(parts) {
            Some(value) => self
                .index
                .as_deref()
                .map_or(IndexSetResult::Unsupported, |index| {
                    index.get_index_set_result(&value)
                }),
            None => IndexSetResult::Unsupported,
        }
    }

    fn get_index_count_for_query_parts(&self, parts: &[&dyn Any]) -> IndexCountResult {
        match P::value_from_query_parts(parts) {
            Some(value) => self
                .index
                .as_deref()
                .map_or(IndexCountResult::Unsupported, |index| {
                    index.get_index_count_result(&value)
                }),
            None => IndexCountResult::Unsupported,
        }
    }
}

/// This function verifies the length invariant of the property's backing storage vector against
/// the entity count according to its `PropertyInitializationKind`. We validate on export as
/// a sanity check and on import to validate serialized data.
fn validate_length<E: Entity, P: Property<E>>(
    entity_count: usize,
    length: usize,
) -> Result<(), IxaError> {
    let valid = match P::initialization_kind() {
        PropertyInitializationKind::Explicit => length == entity_count,
        PropertyInitializationKind::Constant => length <= entity_count,
        PropertyInitializationKind::Derived => false,
    };
    if valid {
        Ok(())
    } else {
        Err(IxaError::InvalidPopulation {
            message: format!(
                "invalid length {length} for property {} of entity {} with population {entity_count}",
                std::any::type_name::<P>(),
                std::any::type_name::<E>()
            ),
        })
    }
}

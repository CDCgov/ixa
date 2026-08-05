/*!

The `PropertyValueStore` trait is the type-erased interface to property value storage.

Responsibilities:

- type-erased interface to the index
- Create "partial" property change events during property value updates

*/

use std::any::Any;

use crate::entity::events::{
    PartialPropertyChangeEventBox, PartialPropertyChangeEventCore, PropertyChangeEvent,
};
use crate::entity::index::{IndexCountResult, IndexSetResult};
use crate::entity::property::Property;
use crate::entity::property_value_store_core::PropertyValueStoreCore;
use crate::entity::{Entity, EntityId};
use crate::Context;

/// The `PropertyValueStore` trait defines the type-erased interface to the concrete property value storage.
pub(crate) trait PropertyValueStore<E: Entity>: Any {
    fn as_any_mut(&mut self) -> &mut dyn Any;
    fn as_any(&self) -> &dyn Any;

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
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn as_any(&self) -> &dyn Any {
        self
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

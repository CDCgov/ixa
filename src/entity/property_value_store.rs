#![allow(unused)]
/*!

The `PropertyValueStore` trait is the type-erased interface to property value storage.

Responsibilities:

- type-erased interface to the index
- Create "partial" property change events during property value updates

*/

use std::any::Any;

use log::{error, trace};

use crate::entity::events::{
    PartialPropertyChangeEvent, PartialPropertyChangeEventBox, PartialPropertyChangeEventCore,
};
use crate::entity::index::{
    FullIndex, IndexCountResult, IndexSetResult, PropertyIndex, PropertyIndexType, ValueCountIndex,
};
use crate::entity::property::Property;
use crate::entity::property_value_store_core::PropertyValueStoreCore;
use crate::entity::{Entity, EntityId};
use crate::hashing::IndexSet;
use crate::{Context, ContextEntitiesExt};

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

    // Index-related methods. Anything beyond these requires the `PropertyValueStoreCore<E, P>`.

    fn get_index_set_for_query_parts(&self, parts: &[&dyn Any]) -> IndexSetResult<'_, E>;

    fn get_index_count_for_query_parts(&self, parts: &[&dyn Any]) -> IndexCountResult;

    /// Returns the index type used by this `PropertyValueStore` instance.
    fn index_type(&self) -> PropertyIndexType;

    /// Sets the index type for this property value store.
    fn set_indexed(&mut self, index_type: PropertyIndexType);

    /// Updates the index for any entities that have been added to the context since the last time the index was
    /// updated.
    fn index_unindexed_entities(&mut self, context: &Context);
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

    fn get_index_set_for_query_parts(&self, parts: &[&dyn Any]) -> IndexSetResult<'_, E> {
        match P::canonical_from_sorted_query_parts(parts) {
            Some(value) => self.index.get_index_set_result(&value),
            None => IndexSetResult::Empty,
        }
    }

    fn get_index_count_for_query_parts(&self, parts: &[&dyn Any]) -> IndexCountResult {
        match P::canonical_from_sorted_query_parts(parts) {
            Some(value) => self.index.get_index_count_result(&value),
            None => IndexCountResult::Count(0),
        }
    }

    fn index_type(&self) -> PropertyIndexType {
        self.index_type()
    }

    fn set_indexed(&mut self, index_type: PropertyIndexType) {
        match index_type {
            PropertyIndexType::Unindexed => {
                self.index = PropertyIndex::Unindexed;
            }
            PropertyIndexType::FullIndex => {
                if self.index.index_type() != PropertyIndexType::FullIndex {
                    self.index = PropertyIndex::FullIndex(FullIndex::new());
                }
            }
            PropertyIndexType::ValueCountIndex => {
                if self.index.index_type() != PropertyIndexType::ValueCountIndex {
                    self.index = PropertyIndex::ValueCountIndex(ValueCountIndex::<E, P>::new());
                }
            }
        }
    }

    fn index_unindexed_entities(&mut self, context: &Context) {
        let current_pop = context.get_entity_count::<E>();
        let max_indexed = match self.index.max_indexed() {
            None => return,
            Some(max_indexed) if max_indexed >= current_pop => return,
            Some(max_indexed) => max_indexed,
        };
        trace!(
            "{}: indexing unindexed entity {}..<{}",
            P::name(),
            max_indexed,
            current_pop
        );

        for id in max_indexed..current_pop {
            let entity_id = EntityId::new(id);
            let value = if P::is_derived() {
                P::compute_derived(context, entity_id)
            } else {
                self.get(entity_id)
            };
            self.index.add_entity(&P::make_canonical(value), entity_id);
        }
        self.index.set_max_indexed(current_pop);
    }
}

#![allow(unused)]
/*!

The `PropertyValueStore` trait is the type-erased interface to property value storage.

Responsibilities:

- type-erased interface to the index
- Create "partial" property change events during property value updates

*/

use std::any::Any;
use std::cell::{Ref, RefCell};

use log::{error, trace};

use crate::entity::events::{PartialPropertyChangeEvent, PartialPropertyChangeEventCore};
use crate::entity::index::{
    FullIndex, IndexCountResult, IndexSetResult, PropertyIndex, PropertyIndexType, ValueCountIndex,
};
use crate::entity::property::Property;
use crate::entity::property_value_store_core::PropertyValueStoreCore;
use crate::entity::{ContextEntitiesExt, Entity, EntityId, HashValueType};
use crate::hashing::IndexSet;
use crate::Context;

/// The `PropertyValueStore` trait defines the type-erased interface to the concrete property value storage.
pub(crate) trait PropertyValueStore<E: Entity>: Any {
    fn as_any_mut(&mut self) -> &mut dyn Any;
    fn as_any(&self) -> &dyn Any;

    // Methods related to updating a value of a dependency
    /// Fetch the existing value of the property for the given `entity_id`, remove the `entity_id`
    /// from the corresponding index bucket, and return a `PartialPropertyChangeEvent` object
    /// wrapping the previous value and `entity_id` that can be used to emit a property change event.
    fn create_partial_property_change(
        &self,
        // The entity_id has been type-erased but is guaranteed by the caller to be an `EntityId<E>`.
        entity_id: EntityId<E>,
        context: &Context,
    ) -> Box<dyn PartialPropertyChangeEvent>;

    // Index-related methods. Anything beyond these requires the `PropertyValueStoreCore<E, P>`.

    fn add_entity_to_index_with_hash(&mut self, hash: HashValueType, entity_id: EntityId<E>);
    fn remove_entity_from_index_with_hash(&mut self, hash: HashValueType, entity_id: EntityId<E>);

    /// Fetches the hash bucket corresponding to the provided hash value. Returns `None` if either the
    /// property is not indexed or there is no bucket corresponding to the hash value.
    fn get_index_set_with_hash(&self, hash: HashValueType) -> Option<Ref<IndexSet<EntityId<E>>>>;

    /// Fetches the hash bucket corresponding to the provided hash value.
    /// Returns `Unsupported` if there is no index or the index cannot return sets.
    fn get_index_set_with_hash_result(
        &self,
        context: &Context,
        hash: HashValueType,
    ) -> IndexSetResult<'_, E>;

    /// Fetches the count corresponding to the provided hash value.
    /// Returns `Unsupported` if there is no index or the index cannot return counts.
    fn get_index_count_with_hash_result(
        &self,
        context: &Context,
        hash: HashValueType,
    ) -> IndexCountResult;

    /// Returns the index type used by this `PropertyValueStore` instance.
    fn index_type(&self) -> PropertyIndexType;

    /// Sets the index type for this property value store.
    fn set_indexed(&mut self, index_type: PropertyIndexType);

    /// Updates the index for any entities that have been added to the context since the last time the index was
    /// updated.
    fn index_unindexed_entities(&self, context: &Context);
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
    ) -> Box<dyn PartialPropertyChangeEvent> {
        // 1. Compute the existing value of the property for the given `entity_id`
        // 2. Remove the `entity_id` from the corresponding index bucket (if its indexed)
        // 3. Return a `PartialPropertyChangeEvent` object wrapping the previous value and `entity_id`.

        let previous_value = if P::is_derived() {
            P::compute_derived(context, entity_id)
        } else {
            self.get(entity_id)
        };
        self.index
            .remove_entity(&previous_value.make_canonical(), entity_id);
        Box::new(PartialPropertyChangeEventCore::<E, P>::new(
            entity_id,
            previous_value,
        ))
    }

    fn add_entity_to_index_with_hash(&mut self, hash: HashValueType, entity_id: EntityId<E>) {
        if self.index.index_type() != PropertyIndexType::Unindexed {
            self.index.add_entity_with_hash(hash, entity_id);
        } else {
            error!("attempted to add an entity to an index for an unindexed property");
        }
    }

    fn remove_entity_from_index_with_hash(&mut self, hash: HashValueType, entity_id: EntityId<E>) {
        if self.index.index_type() != PropertyIndexType::Unindexed {
            self.index.remove_entity_with_hash(hash, entity_id);
        } else {
            error!("attempted to remove an entity from an index for an unindexed property");
        }
    }

    fn get_index_set_with_hash(&self, hash: HashValueType) -> Option<Ref<IndexSet<EntityId<E>>>> {
        self.index.get_index_set_with_hash(hash)
    }

    fn get_index_set_with_hash_result(
        &self,
        context: &Context,
        hash: HashValueType,
    ) -> IndexSetResult<'_, E> {
        self.index_unindexed_entities(context);
        self.index.get_index_set_with_hash_result(hash)
    }

    fn get_index_count_with_hash_result(
        &self,
        context: &Context,
        hash: HashValueType,
    ) -> IndexCountResult {
        self.index_unindexed_entities(context);
        self.index.get_index_count_with_hash_result(hash)
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
                    self.index = PropertyIndex::FullIndex(RefCell::new(FullIndex::new()));
                }
            }
            PropertyIndexType::ValueCountIndex => {
                if self.index.index_type() != PropertyIndexType::ValueCountIndex {
                    self.index =
                        PropertyIndex::ValueCountIndex(
                            RefCell::new(ValueCountIndex::<E, P>::new()),
                        );
                }
            }
        }
    }

    fn index_unindexed_entities(&self, context: &Context) {
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
            let value = context.get_property::<E, P>(entity_id);
            self.index.add_entity(&P::make_canonical(value), entity_id);
        }
        self.index.set_max_indexed(current_pop);
    }
}

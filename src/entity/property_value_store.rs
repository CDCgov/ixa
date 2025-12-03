#![allow(unused)]
/*!

The `PropertyValueStore` trait is the type-erased interface to property value storage.

Responsibilities:

- type-erased interface to the index
- Create "partial" property change events during property value updates

*/

use std::any::Any;

use log::{error, trace};

use crate::{
    entity::{
        index::Index,
        property::Property,
        property_value_store_core::PropertyValueStoreCore,
        Entity,
        EntityId,
        HashValueType
    },
    Context,
    HashSet
};
use crate::entity::events::{PartialPropertyChangeEvent, PartialPropertyChangeEventCore};

/// The `PropertyValueStore` trait defines the type-erased interface to the concrete property value storage.
pub trait PropertyValueStore<E: Entity> {
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any;

    // Methods related to updating a value of a dependency
    /// Fetch the existing value of the property for the given `entity_id`, remove the `entity_id`
    /// from the corresponding index bucket, and return a `PartialPropertyChangeEvent` object
    /// wrapping the previous value and `entity_id` that can be used to emit a property change event.
    fn create_partial_property_change(&mut self, entity_id: EntityId<E>) -> Box<dyn PartialPropertyChangeEvent>;

    // Index-related methods. Anything beyond these requires the `PropertyValueStoreCore<E, P>`.
    fn add_entity_to_index_with_hash(&mut self, hash: HashValueType, entity_id: EntityId<E>);
    fn remove_entity_from_index_with_hash(&mut self, hash: HashValueType, entity_id: EntityId<E>);
    fn get_index_set_with_hash(&self, hash: HashValueType) -> Option<&HashSet<EntityId<E>>>;
    fn is_indexed(&self) -> bool;
    fn set_indexed(&mut self, is_indexed: bool);
    fn index_unindexed_entities(&mut self, context: &Context);
}

impl<E: Entity, P: Property<E>> PropertyValueStore<E> for PropertyValueStoreCore<E, P> {
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn create_partial_property_change(&mut self, entity_id: EntityId<E>) -> Box<dyn PartialPropertyChangeEvent> {
        // 1. Fetch the existing value of the property for the given `entity_id`
        // 2. Remove the `entity_id` from the corresponding index bucket
        // 3. Return a `PartialPropertyChangeEvent` object wrapping the previous value and `entity_id`.
        // ToDo(RobertJacobsonCDC): Is this always `Some`? Is this unwrap justified?
        let previous_value = self.get(entity_id).unwrap();
        if let Some(index) = &mut self.index {
            index.remove_entity(&previous_value.make_canonical(), entity_id);
        }
        Box::new(PartialPropertyChangeEventCore::<E, P>::new(entity_id, previous_value))
    }

    fn add_entity_to_index_with_hash(&mut self, hash: HashValueType, entity_id: EntityId<E>) {
        if let Some(index) = &mut self.index {
            index.add_entity_with_hash(hash, entity_id);
        } else {
            error!("attempted to add an entity to an index for an unindexed property");
        }
    }

    fn remove_entity_from_index_with_hash(&mut self, hash: HashValueType, entity_id: EntityId<E>) {
        if let Some(index) = &mut self.index {
            index.remove_entity_with_hash(hash, entity_id);
        } else {
            error!("attempted to remove an entity from an index for an unindexed property");
        }
    }

    fn get_index_set_with_hash(&self, hash: HashValueType) -> Option<&HashSet<EntityId<E>>> {
        if let Some(index) = &self.index {
            index.get_with_hash(hash)
        } else {
            error!("attempted to add an entity to a property index for an unindexed property");
            None
        }
    }

    fn is_indexed(&self) -> bool {
        self.index.is_some()
    }

    fn set_indexed(&mut self, is_indexed: bool) {
        if is_indexed && !self.is_indexed() {
            self.index = Some(Index::new());
        } else if !is_indexed {
            self.index = None;
        }
    }

    fn index_unindexed_entities(&mut self, context: &Context) {
        if let Some(index) = &mut self.index {
            let current_pop = context.get_entity_count::<E>();
            trace!(
                "{}: indexing unindexed entity {}..<{}",
                P::name(),
                index.max_indexed,
                current_pop
            );

            for id in index.max_indexed..current_pop {
                let entity_id = EntityId::new(id);
                let value = context.get_property(entity_id);
                index.add_entity(&P::make_canonical(value), entity_id);
            }
            index.max_indexed = current_pop;
        }
    }
}

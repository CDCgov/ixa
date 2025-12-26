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
use crate::entity::index::Index;
use crate::entity::property::Property;
use crate::entity::property_value_store_core::PropertyValueStoreCore;
use crate::entity::{Entity, EntityId, HashValueType};
use crate::{Context, HashSet};

/// The `PropertyValueStore` trait defines the type-erased interface to the concrete property value storage.
pub(crate) trait PropertyValueStore: Any {
    fn as_any_mut(&mut self) -> &mut dyn Any;
    fn as_any(&self) -> &dyn Any;

    // Methods related to updating a value of a dependency
    /// Fetch the existing value of the property for the given `entity_id`, remove the `entity_id`
    /// from the corresponding index bucket, and return a `PartialPropertyChangeEvent` object
    /// wrapping the previous value and `entity_id` that can be used to emit a property change event.
    fn create_partial_property_change(
        &self,
        // The entity_id has been type-erased but is guaranteed by the caller to be an `EntityId<E>`.
        entity_id: usize,
        context: &Context,
    ) -> Box<dyn PartialPropertyChangeEvent>;

    // Index-related methods. Anything beyond these requires the `PropertyValueStoreCore<E, P>`.

    fn add_entity_to_index_with_hash(&mut self, hash: HashValueType, entity_id: usize);
    fn remove_entity_from_index_with_hash(&mut self, hash: HashValueType, entity_id: usize);
    // fn get_index_set_with_hash(&self, hash: HashValueType) -> Option<Ref<HashSet<EntityId<E>>>>;

    /// Returns whether this `PropertyValueStore` instance has an `Index<E, P>`. Note that this is not the same as
    /// asking whether the property itself is indexed, as some properties might use the index of some other
    /// `PropertyValueStore`, as in the case of multi-properties.
    fn is_indexed(&self) -> bool;

    /// If `is_indexed` is `true`, constructs an `Index<E, P>` if one doesn't already exist. If `is_indexed` is `false`,
    /// sets `self.index` to `None`, dropping any existing index.
    fn set_indexed(&mut self, is_indexed: bool);

    /// Updates the index for any entities that have been added to the context since the last time the index was updated.
    fn index_unindexed_entities(&mut self, context: &Context);
}

impl<E: Entity, P: Property<E>> PropertyValueStore for PropertyValueStoreCore<E, P> {
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn create_partial_property_change(
        &self,
        entity_id: usize,
        context: &Context,
    ) -> Box<dyn PartialPropertyChangeEvent> {
        // 1. Compute the existing value of the property for the given `entity_id`
        // 2. Remove the `entity_id` from the corresponding index bucket (if its indexed)
        // 3. Return a `PartialPropertyChangeEvent` object wrapping the previous value and `entity_id`.

        // The entity_id has been type-erased but is guaranteed by the caller to be an `EntityId<E>`.
        let entity_id = EntityId::<E>::new(entity_id);

        let previous_value = P::compute_derived(context, entity_id);
        if let Some(index) = &self.index {
            let mut index = index.borrow_mut();
            index.remove_entity(&previous_value.make_canonical(), entity_id);
        }
        Box::new(PartialPropertyChangeEventCore::<E, P>::new(
            entity_id,
            previous_value,
        ))
    }

    fn add_entity_to_index_with_hash(&mut self, hash: HashValueType, entity_id: usize) {
        // The entity_id has been type-erased but is guaranteed by the caller to be an `EntityId<E>`.
        let entity_id = EntityId::<E>::new(entity_id);

        if let Some(index) = &mut self.index {
            index.get_mut().add_entity_with_hash(hash, entity_id);
        } else {
            error!("attempted to add an entity to an index for an unindexed property");
        }
    }

    fn remove_entity_from_index_with_hash(&mut self, hash: HashValueType, entity_id: usize) {
        // The entity_id has been type-erased but is guaranteed by the caller to be an `EntityId<E>`.
        let entity_id = EntityId::<E>::new(entity_id);

        if let Some(index) = &mut self.index {
            index.get_mut().remove_entity_with_hash(hash, entity_id);
        } else {
            error!("attempted to remove an entity from an index for an unindexed property");
        }
    }

    // fn get_index_set_with_hash(&self, hash: HashValueType) -> Option<Ref<HashSet<EntityId<E>>>> {
    //     if let Some(index) = &self.index {
    //         let index = index.borrow();
    //         Ref::filter_map(index, |idx| idx.get_with_hash(hash)).ok()
    //     } else {
    //         error!("attempted to add an entity to a property index for an unindexed property");
    //         None
    //     }
    // }

    fn is_indexed(&self) -> bool {
        self.index.is_some()
    }

    fn set_indexed(&mut self, is_indexed: bool) {
        if is_indexed && !self.is_indexed() {
            self.index = Some(RefCell::new(Index::new()));
        } else if !is_indexed {
            self.index = None;
        }
    }

    fn index_unindexed_entities(&mut self, context: &Context) {
        if let Some(index) = &mut self.index {
            let index = index.get_mut();
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

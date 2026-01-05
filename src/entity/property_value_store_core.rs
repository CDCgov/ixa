/*!

A
`PropertyValueStoreCore<E: Entity, P: Property<E>>` is the concrete type
implementing the value storage.

- Gets values
- Sets values while:
    - maintaining the index and
    - emitting property change events
- Gives access to iterators over query results (`SourceSet` instances that abstract over index set vs. non-index set)
- Integrates the functionality of `TypeErasedIndex`.

*/

use std::cell::RefCell;

use super::entity::{Entity, EntityId};
use super::index::Index;
use super::property::{Property, PropertyInitializationKind};
use crate::entity::property_value_store::PropertyValueStore;
use crate::value_vec::ValueVec;

/// The underlying storage type for property values.
pub(crate) type RawPropertyValueVec<P> = ValueVec<Option<P>>;

pub struct PropertyValueStoreCore<E: Entity, P: Property<E>> {
    /// The backing storage vector for the property. Always empty if the property is derived.
    pub(super) data: RawPropertyValueVec<P>,
    /// An index mapping `property_value` to `set_of_entities`.
    // Note that while we use a `RefCell` here, most of the time we don't incur the overhead of dynamic borrow checking,
    // because we use `index.get_mut()` instead of `index.borrow_mut()`. We only need `index.borrow_mut()` for
    // updating the index during setting of a property, at which time the compiler guarantees no other borrows of
    // `context` exist because `Context::set_property` takes `&mut self`.
    pub(super) index: Option<RefCell<Index<E, P>>>,
}

impl<E: Entity, P: Property<E>> Default for PropertyValueStoreCore<E, P> {
    fn default() -> Self {
        Self {
            data: ValueVec::default(),
            index: Default::default(),
        }
    }
}

impl<E: Entity, P: Property<E>> PropertyValueStoreCore<E, P> {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn new_boxed() -> Box<dyn PropertyValueStore<E>> {
        Box::new(Self::new())
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            data: ValueVec::with_capacity(capacity),
            index: None,
        }
    }

    /// Ensures capacity for at least `additional` more elements
    pub fn reserve(&self, additional: usize) {
        self.data.reserve(additional);
    }

    /// Returns the property value for the given entity. Returns `None`
    /// if the property is both not set and has no default value.
    pub fn get(&self, entity_id: EntityId<E>) -> Option<P> {
        self.data.get(entity_id.0).unwrap_or_else(|| {
            // `None` means index was out of bounds, which means the property is not set.
            // Return the default if there is one.
            if P::initialization_kind() == PropertyInitializationKind::Constant {
                Some(P::default_const())
            } else {
                None
            }
        })
    }

    /// Sets the value for `entity_id` to `value`.
    pub fn set(&self, entity_id: EntityId<E>, value: P) {
        let index = entity_id.0;
        let len = self.data.len();

        if index >= len {
            // The index is out of bounds. We potentially expand the backing storage with default values.
            let default_value = match P::initialization_kind() {
                PropertyInitializationKind::Constant => Some(P::default_const()),
                _ => None,
            };

            // If we are trying to set the same value as the default, don't bother doing anything.
            if Some(value) == default_value {
                return;
            }

            // Pre-reserve exact capacity to avoid reallocations
            self.data.reserve(index + 1 - len);

            // Fill any missing slots up to (but not including) `idx`
            self.data.resize_with(index, || default_value.clone());
            // ...and finally push the provided value
            self.data.push(Some(value));
        } else {
            // The index is in bounds, so we can just set the value directly.
            self.data.set(index, Some(value));
        }
    }

    /// Sets the value for `entity_id` to `value`, returning the previous value if it exists.
    pub fn replace(&self, entity_id: EntityId<E>, value: P) -> Option<P> {
        let index = entity_id.0;
        let len = self.data.len();

        if index >= len {
            // The index is out of bounds. We potentially expand the backing storage with default values.
            let default_value = match P::initialization_kind() {
                PropertyInitializationKind::Constant => Some(P::default_const()),
                _ => None,
            };

            // If we are trying to set the same value as the default, don't bother doing anything.
            if Some(value) == default_value {
                return Some(value);
            }

            // Pre-reserve exact capacity to avoid reallocations
            self.data.reserve(index + 1 - len);

            // Fill any missing slots up to (but not including) `idx`
            self.data.resize_with(index, || default_value.clone());
            // ...and finally push the provided value
            self.data.push(Some(value));

            // The "existing value" is the default.
            default_value
        } else {
            // The index is in bounds, so we can just set the value directly.
            self.data.replace(index, Some(value))
        }
    }
}

// See tests in `property_store.rs`.

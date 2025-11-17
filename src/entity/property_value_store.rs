/*!

A `PropertyStore<P: Property>` is the backing storage for property values.

*/

use super::entity::{Entity, EntityId};
use super::property::{Property, PropertyInitializationKind};
use crate::value_vec::ValueVec;

pub struct PropertyValueStore<E: Entity, P: Property<E>> {
    data: ValueVec<Option<P>>,

    _phantom: std::marker::PhantomData<E>,
}

impl<E: Entity, P: Property<E>> Default for PropertyValueStore<E, P> {
    fn default() -> Self {
        Self {
            data: ValueVec::default(),
            _phantom: Default::default(),
        }
    }
}

impl<E: Entity, P: Property<E>> PropertyValueStore<E, P> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            data: ValueVec::with_capacity(capacity),
            _phantom: Default::default(),
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
            // The index is out of bounds, so we need to fill in the missing slots.
            let default_value = match P::initialization_kind() {
                PropertyInitializationKind::Constant => Some(P::default_const()),
                _ => None,
            };

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
}

// See tests in `property_store.rs`.

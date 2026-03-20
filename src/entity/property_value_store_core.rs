/*!

A `PropertyValueStoreCore<E: Entity, P: Property<E>>` is the concrete type implementing the value storage.

This concrete type exists primarily to own the backing storage for non-derived properties and the `Index<E, P>`
instance, if there is one. It implements only the lowest level getters and setters for the property storage.
A higher level interface to the `Index` is provided through the `PropertyValueStore<E>` trait.

*/

use std::cell::RefCell;

use super::entity::{Entity, EntityId};
use super::property::{Property, PropertyInitializationKind};
use crate::entity::index::{PropertyIndex, PropertyIndexType};
use crate::entity::property_value_store::PropertyValueStore;
use crate::entity::value_change_counter::ValueChangeCounter;
use crate::value_vec::ValueVec;

/// The underlying storage type for property values.
pub(crate) type RawPropertyValueVec<P> = ValueVec<P>;

pub struct PropertyValueStoreCore<E: Entity, P: Property<E>> {
    /// The backing storage vector for the property. Always empty if the property is derived.
    pub(super) data: RawPropertyValueVec<P>,
    /// An index mapping `property_value` to `set_of_entities`.
    pub(crate) index: PropertyIndex<E, P>,
    /// Value change counters for this property.
    pub(crate) value_change_counters: Vec<RefCell<Box<dyn ValueChangeCounter<E, P>>>>,
}

impl<E: Entity, P: Property<E>> Default for PropertyValueStoreCore<E, P> {
    fn default() -> Self {
        Self {
            data: ValueVec::default(),
            index: PropertyIndex::Unindexed,
            value_change_counters: Vec::new(),
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
            index: PropertyIndex::Unindexed,
            value_change_counters: Vec::new(),
        }
    }

    pub(crate) fn index_type(&self) -> PropertyIndexType {
        self.index.index_type()
    }

    /// Adds a value change counter and returns its ID.
    pub(crate) fn add_value_change_counter(
        &mut self,
        counter: Box<dyn ValueChangeCounter<E, P>>,
    ) -> usize {
        let counter_id = self.value_change_counters.len();
        self.value_change_counters.push(RefCell::new(counter));
        counter_id
    }

    /// Ensures capacity for at least `additional` more elements
    pub fn reserve(&self, additional: usize) {
        self.data.reserve(additional);
    }

    /// Returns the property value for the given entity.
    pub fn get(&self, entity_id: EntityId<E>) -> P {
        debug_assert!(
            !P::is_derived(),
            "Tried to get a derived property value from property value store."
        );
        self.data.get(entity_id.0).unwrap_or_else(
            // `None` means index was out of bounds, which means the property has not been set.
            // Return the default.
            P::default_const,
        )
    }

    /// Sets the value for `entity_id` to `value`.
    pub fn set(&self, entity_id: EntityId<E>, value: P) {
        debug_assert!(
            !P::is_derived(),
            "Tried to set a derived property value in property value store."
        );
        let index = entity_id.0;
        let len = self.data.len();

        if index < len {
            // The index is in bounds, so we can just set the value directly.
            self.data.set(index, value);
            return;
        }

        // The index is out of bounds.

        if P::initialization_kind() == PropertyInitializationKind::Constant {
            // When a default constant value exists, we implement the optimization that we don't have to store those
            // default values.
            let default_value = P::default_const();

            // If we are trying to set the same value as the default, don't bother doing anything.
            if value == default_value {
                return;
            }

            // Pre-reserve exact capacity to avoid reallocations
            self.data.reserve(index + 1 - len);

            // Fill any missing slots up to (but not including) `index`
            self.data.resize(index, default_value);
            // ...and finally push the provided value
            self.data.push(value);
        } else if index == len {
            // This case occurs when adding a new entity. The optimization for default constants does not apply.
            self.data.push(value);
        } else {
            // No default property value, and we are trying to set a value for an index past the end of the vector.
            // This is an internal error, as we enforce the invariant that every property must have a value.
            unreachable!("Property storage state is inconsistent: one or more properties do not have values.");
        }
    }

    /// Sets the value for `entity_id` to `value`, returning the previous value.
    pub fn replace(&self, entity_id: EntityId<E>, value: P) -> P {
        debug_assert!(
            !P::is_derived(),
            "Tried to replace a derived property value in property value store."
        );
        let index = entity_id.0;
        let len = self.data.len();

        if index < len {
            // The index is in bounds, so we can just set the value directly.
            return self.data.replace(index, value);
        }

        // The index is out of bounds.

        if P::initialization_kind() == PropertyInitializationKind::Constant {
            // When a default constant value exists, we implement the optimization that we don't have to store those
            // default values.
            let default_value = P::default_const();

            // If we are trying to set the same value as the default, don't bother doing anything.
            if value == default_value {
                return default_value;
            }

            // Pre-reserve exact capacity to avoid reallocations
            self.data.reserve(index + 1 - len);

            // Fill any missing slots up to (but not including) `index`
            self.data.resize(index, default_value);
            // ...and finally push the provided value
            self.data.push(value);

            // The "existing value" is the default.
            default_value
        } else {
            // No default property value, and we are trying to set a value for an index past the end of the vector.
            // This is an internal error, as we enforce the invariant that every property must have a value.
            unreachable!("Property storage state is inconsistent: one or more properties do not have values.");
        }
    }

    /// Writes a contiguous run of values starting at `start_index`.
    ///
    /// This has a fast path for appending contiguous rows (`start_index == len`) to
    /// avoid per-value bounds/default branching in `set`.
    pub fn set_contiguous_from_rows<T, F>(&self, start_index: usize, rows: &[T], get_value: F)
    where
        F: Copy + Fn(&T) -> P,
    {
        debug_assert!(
            !P::is_derived(),
            "Tried to set a derived property value in property value store."
        );
        if rows.is_empty() {
            return;
        }

        let len = self.data.len();

        // Fast path: contiguous append.
        if start_index == len {
            if P::initialization_kind() == PropertyInitializationKind::Constant {
                let default_value = P::default_const();
                let mut has_default = false;
                let mut has_non_default = false;

                for row in rows {
                    if get_value(row) == default_value {
                        has_default = true;
                    } else {
                        has_non_default = true;
                    }
                    if has_default && has_non_default {
                        break;
                    }
                }

                // All values are default; keep sparse optimization.
                if !has_non_default {
                    return;
                }

                // No defaults in this chunk; append directly in one pass.
                if !has_default {
                    self.data.reserve(rows.len());
                    self.data.extend(rows.iter().map(get_value));
                    return;
                }
            } else {
                self.data.reserve(rows.len());
                self.data.extend(rows.iter().map(get_value));
                return;
            }
        }

        // General fallback path.
        for (offset, row) in rows.iter().enumerate() {
            self.set(EntityId::new(start_index + offset), get_value(row));
        }
    }

    /// Writes `count` copies of `value` starting at `start_index`.
    ///
    /// This has an append fast path and preserves the sparse optimization for constant default
    /// properties when the repeated value is the default.
    pub fn set_contiguous_repeated(&self, start_index: usize, count: usize, value: P) {
        debug_assert!(
            !P::is_derived(),
            "Tried to set a derived property value in property value store."
        );
        if count == 0 {
            return;
        }

        let len = self.data.len();

        if start_index >= len && P::initialization_kind() == PropertyInitializationKind::Constant {
            let default_value = P::default_const();

            if value == default_value {
                return;
            }

            self.data.reserve(start_index + count - len);
            self.data.resize(start_index, default_value);
            self.data.resize(start_index + count, value);
            return;
        }

        if start_index == len {
            self.data.reserve(count);
            self.data.resize(start_index + count, value);
            return;
        }

        for offset in 0..count {
            self.set(EntityId::new(start_index + offset), value);
        }
    }
}

// See tests in `property_store.rs`.

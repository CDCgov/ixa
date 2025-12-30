#![allow(dead_code)]
//! Utilities for managing and querying multi-properties.
//!
//! A multi-property is a derived property composed of a tuple of other properties. They are
//! primarily used to enable joint indexing and efficient multi-column style queries.
//!
//! ## Indexing and Normalization
//!
//! To ensure that queries can efficiently find matching indexes, multi-properties that consist
//! of the same set of component properties are considered equivalent, regardless of their
//! definition order. The system achieves this by:
//! 1.  **Canonicalization**: Component properties are sorted based on their unique identifiers.
//! 2.  **Shared Indexes**: The index subsystem reuses a single [`Index`] instance for all
//!     multi-properties that are equivalent up to reordering.
//!
//! ## Query Integration
//!
//! The querying subsystem uses the utilities in this module to detect when a query involving
//! multiple individual properties can be satisfied by an existing multi-property index. If a
//! match is found, the query can perform a fast index lookup instead of iterating over component
//! properties.
//!
//! ## Implementation Details
//!
//! Multi-properties are defined using the [`define_multi_property!`] macro, which
//! handles the registration and mapping between the property set and its canonical
//! index ID. This module provides the runtime registry ([`MULTI_PROPERTY_INDEX_MAP`])
//! and reordering logic used by both the macro-generated code and the query engine.

use std::any::TypeId;
use std::cell::RefCell;
use std::sync::{LazyLock, Mutex};

use crate::hashing::{one_shot_128, HashMap, HashValueType};

/// A map from a list of `TypeId`s to the index ID of the equivalent multi-property
/// type. The list of `TypeId`s is assumed to be sorted.
///
/// Use `register_type_ids_to_muli_property_index()` to register a multi-property.
/// We could instead just rely on `TypeId::of::<P::CanonicalValue>()`, but this
/// allows us to determine the type dynamically, e.g. for the web API or debug
/// console.
static MULTI_PROPERTY_INDEX_MAP: LazyLock<Mutex<RefCell<HashMap<HashValueType, usize>>>> =
    LazyLock::new(|| Mutex::new(RefCell::new(HashMap::default())));

/// A method that looks up the index ID of the multi-property that has the given
/// list of `TypeId`s as its properties.
pub fn type_ids_to_multi_property_index(type_ids: &[TypeId]) -> Option<usize> {
    let hash = one_shot_128(&type_ids);
    MULTI_PROPERTY_INDEX_MAP
        .lock()
        .unwrap()
        .borrow()
        .get(&hash)
        .copied()
}

/// A method that registers the index ID of the multi-property tuple type that has the given
/// list of `TypeId`s as its properties.
///
/// Use `type_ids_to_muli_property_index()` to look up an index ID.
pub fn register_type_ids_to_muli_property_index(type_ids: &[TypeId], index: usize) {
    let hash = one_shot_128(&type_ids);
    MULTI_PROPERTY_INDEX_MAP
        .lock()
        .unwrap()
        .borrow_mut()
        .insert(hash, index);
}

// The following free functions are utilities used in the macro implementation of `Query::multi_property_value_hash`,
// which computes the same hash as an equivalent multi-property. This requires the values to be ordered according to
// the lexicographic order of the property type names, which for queries must be done dynamically, as queries do
// not (and cannot) have a proc macro trait impl.

/// An iota function that returns an array of the form `[0, 1, 2, 3, ..., N-1]`. The size of the array
/// is statically known, avoiding `Vec` allocations.
const fn make_indices<const N: usize>() -> [usize; N] {
    let mut arr = [0; N];
    let mut i = 0;
    while i < N {
        arr[i] = i;
        i += 1;
    }
    arr
}

/// Returns the indices of `keys` in sorted order. These indices are used to reorder some other
/// array according to the sorted order of the `keys`, e.g. by `static_apply_reordering`.
///
/// "Static" in the name refers to the fact that it takes and returns an array of statically
/// known size, avoiding `Vec` allocations.
pub fn static_sorted_indices<T: Ord, const N: usize>(keys: &[T; N]) -> [usize; N] {
    let mut indices = make_indices::<N>();
    indices.sort_by_key(|&i| &keys[i]);
    indices
}

/// Reorders the `values` in place according to the ordering defined by `indices`. The `indices`
/// is an ordering produced by `sorted_indices`/`static_sorted_indices` and encodes the sorted
/// order of the `keys` (the names of the tag types).
///
/// "Static" in the name refers to the fact that it takes and returns an array of statically
/// known size, avoiding `Vec` allocations.
pub fn static_apply_reordering<T: Copy, const N: usize>(values: &mut [T; N], indices: &[usize; N]) {
    let tmp_values: [T; N] = *values;
    for (old_index, new_index) in indices.iter().enumerate() {
        values[old_index] = tmp_values[*new_index];
    }
}

/// Reorder `values` in place according to the sorted order of `keys`.
///
/// Both slices must have the same length. "Static" in the name refers to the fact that it
/// takes and returns an array of statically known size, avoiding `Vec` allocations.
pub fn static_reorder_by_keys<T: Ord + Copy, U: Copy, const N: usize>(
    keys: &[T; N],
    values: &mut [U; N],
) {
    let indices: [usize; N] = static_sorted_indices(keys);
    static_apply_reordering(values, &indices);
}

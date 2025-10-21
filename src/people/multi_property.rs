//! The utilities in this module are used by query and multi-properties so that queries
//! having multiple properties can be resolved to an indexed multi-property if possible.

use std::any::TypeId;
use std::cell::RefCell;
use std::sync::{LazyLock, Mutex};

use crate::hashing::{one_shot_128, HashMap};
use crate::people::HashValueType;

/// A map from a list of `TypeId`s to the `TypeId` of the multi-property type.
/// The list of `TypeId`s is assumed to be sorted.
///
/// Use `register_type_ids_to_muli_property_id()` to register a multi-property.
static MULTI_PROPERTY_INDEX_MAP: LazyLock<Mutex<RefCell<HashMap<HashValueType, TypeId>>>> =
    LazyLock::new(|| Mutex::new(RefCell::new(HashMap::default())));

/// A method that looks up the `TypeId` of the multi-property that has the given
/// list of `TypeId`s as its properties.
pub fn type_ids_to_multi_property_id(type_ids: &[TypeId]) -> Option<TypeId> {
    let hash = one_shot_128(&type_ids);
    MULTI_PROPERTY_INDEX_MAP
        .lock()
        .unwrap()
        .borrow()
        .get(&hash)
        .copied()
}

/// A method that registers the `TypeId` of the multi-property that has the given
/// list of `TypeId`s as its properties.
///
/// Use `type_ids_to_muli_property_id()` to look up a `TypeId`.
pub fn register_type_ids_to_muli_property_id(type_ids: &[TypeId], multi_property_id: TypeId) {
    let hash = one_shot_128(&type_ids);
    MULTI_PROPERTY_INDEX_MAP
        .lock()
        .unwrap()
        .borrow_mut()
        .insert(hash, multi_property_id);
}

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
/// array according to the sorted order of the `keys`.
///
/// This version works in cases where the size of the slice is not known at compile time,
/// returning an allocated `Vec`.
pub fn sorted_indices<T: Ord>(keys: &[T]) -> Vec<usize> {
    let mut indices: Vec<usize> = (0..keys.len()).collect();
    indices.sort_by_key(|&i| &keys[i]);
    indices
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

/// Reorder `values` according to the sorted order of `keys`.
///
/// Both slices are assumed to have the same length. This version works in cases where
/// the size of the slice is not known at compile time, returning an allocated `Vec`.
pub fn reorder_by_keys<T: Ord + Copy, U: Copy>(keys: &mut [T], values: &mut [U]) {
    let indices: Vec<usize> = sorted_indices(keys);
    let tmp_keys = Vec::from(&*keys);
    let tmp_values = Vec::from(&*values);

    // Apply the permutation to values
    for (old_index, new_index) in indices.into_iter().enumerate() {
        values[old_index] = tmp_values[new_index];
        keys[old_index] = tmp_keys[new_index];
    }
}

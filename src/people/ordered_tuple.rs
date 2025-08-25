#![allow(dead_code)]
/*!
The `define_multi_property!` macro (defined in `property.rs`) takes a name
and a tuple of property tags and implements `SortByTag` for the tuple. It
defines a derived property (vio `define_derived_property`) with the provided
name having the type of tuples of values corresponding to the provided tags.

```rust,ignore
use ixa::people::{define_multi_property, define_person_property};

define_person_property!(Name, &'static str);
define_person_property!(Age, u8);
define_person_property!(Weight, f64);

define_multi_property!(Profile, (Name, Age, Weight));
```

The new derived property is not automatically indexed. You can index it just
like any other property:

```rust, ignore
context.index_property(Profile);
```

The `SortByTag` trait endows the tuple type with methods `reorder_by_tag`
and `unreorder_by_tag` that converts to and from the tuple value type
of the sorted order of the fields of the tuple `Tag` respectively.
*/

use crate::hashing::{one_shot_128, HashMap};
use crate::people::HashValueType;
use std::any::TypeId;
use std::cell::RefCell;
use std::sync::{LazyLock, Mutex};

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

/// Orders the fields of the tuple `Self` according to the sorted order of the
/// fields of the tuple `Tag`. The `SortedTag` associated type is the tuple type of
/// `Tag` after sorting. The `ReorderedValue` associated type is the tuple type of
/// `Self` after reordering, according to the sorted order of the fields of `Tag`.
///
/// This can be automatically implemented with the syntax
/// `ixa_derive::sorted_tag_value_impl!(tag_tuple = (...), value_tuple = (...));`.
pub trait SortByTag<Tag> {
    // The unsorted tag type is available as of course `Tag`.
    type SortedTag;
    // The unsorted value type is available as `Tag::Value`.
    type ReorderedValue;

    /// Converts a `Self` value to a `Self::ReorderedValue` value.
    fn reorder_by_tag(self) -> Self::ReorderedValue;
    /// The inverse of `reorder_by_tag`. Note that this is an associated function, not a method.
    fn unreorder_by_tag(sorted: Self::ReorderedValue) -> Self;
}

/// Implements `SortByTag<(Property1, Property2, ...)>`, where the `Tag` is a tuple of properties.
#[macro_export]
macro_rules! sorted_property_impl {
    (
        ( $($dependency:ident),+ )
    ) => {
        ixa_derive::sorted_tag_value_impl!(
            tag_tuple = ( $($dependency),+ ),
            value_tuple = ( $(<$dependency as $crate::people::PersonProperty>::Value),+ )
        );
    };
}
#[allow(unused_imports)]
pub use sorted_property_impl;

const fn make_indices<const N: usize>() -> [usize; N] {
    let mut arr = [0; N];
    let mut i = 0;
    while i < N {
        arr[i] = i;
        i += 1;
    }
    arr
}

/// Returns the indices of `keys` in sorted order.
pub fn sorted_indices<T: Ord>(keys: &[T]) -> Vec<usize> {
    let mut indices: Vec<usize> = (0..keys.len()).collect();
    indices.sort_by_key(|&i| &keys[i]);
    indices
}

/// Returns the indices of `keys` in sorted order. Does not allocate.
pub fn static_sorted_indices<T: Ord, const N: usize>(keys: &[T; N]) -> [usize; N] {
    let mut indices = make_indices::<N>();
    indices.sort_by_key(|&i| &keys[i]);
    indices
}

pub fn static_apply_reordering<T: Copy, const N: usize>(values: &mut [T; N], indices: &[usize; N]) {
    let tmp_values: [T; N] = *values;
    for (old_index, new_index) in indices.iter().enumerate() {
        values[old_index] = tmp_values[*new_index];
    }
}

/// Reorder `values` according to the sorted order of `keys`.
/// Both slices must have the same length. Does not allocate.
pub fn static_reorder_by_keys<T: Ord + Copy, U: Copy, const N: usize>(
    keys: &mut [T; N],
    values: &mut [U; N],
) {
    let indices: [usize; N] = static_sorted_indices(&*keys);
    let tmp_keys: [T; N] = *keys;
    let tmp_values: [U; N] = *values;

    // Apply the permutation to values
    for (old_index, new_index) in indices.into_iter().enumerate() {
        values[old_index] = tmp_values[new_index];
        keys[old_index] = tmp_keys[new_index];
    }
}

/// Reorder `values` according to the sorted order of `keys`.
/// Both slices are assumed to have the same length.
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

#[cfg(test)]
mod tests {
    use super::*;
    use ixa_derive::sorted_tag_value_impl;

    struct TagA;
    struct TagB;
    struct TagC;

    // The macro is written generically in case we have other use cases. For multi-indexes,
    // we would generate the following:
    // ```rust,ignore
    //   sorted_tag_value_impl!(
    //     tag_tuple = (TagC, TagA, TagB),
    //     value_tuple = (TagC::Value, TagA::Value, TagB::Value)
    //   );
    // ```
    sorted_tag_value_impl!(
      tag_tuple = (TagC, TagA, TagB),
      value_tuple = (u8, &'static str, f64)
    );

    // The macro above generates the following:
    // ```rust
    // impl SortByTag<(TagC, TagA, TagB)> for (u8, &'static str, f64) {
    //   type SortedTag = (TagA, TagB, TagC);
    //   type ReorderedValue = (&'static str, f64, u8);
    //   fn reorder_by_tag(self) -> Self::ReorderedValue {
    //     let (t0, t1, t2) = self;
    //     (t1, t2, t0)
    //   }
    //   fn unreorder_by_tag(sorted: Self::ReorderedValue) -> Self {
    //     let (s0, s1, s2) = sorted;
    //     (s2, s0, s1)
    //   }
    // }
    // ```

    #[test]
    fn test_sort_by_tag() {
        let values = (123u8, "hi", 3.14);
        let sorted = values.reorder_by_tag();
        // You would need a type annotation if the types were ambiguous.
        // let sorted = <_ as SortByTag<(TagC, TagA, TagB)>>::reorder_by_tag(values);
        let expected_sorted = ("hi", 3.14, 123);
        let unsorted = <(u8, &'static str, f64)>::unreorder_by_tag(expected_sorted);
        // You would need the full type annotation if the types were ambiguous.
        // let unsorted = <(u8, &'static str, f64) as SortByTag<(TagC, TagA, TagB)>>::unreorder_by_tag(expected_sorted);

        println!(
            "expected: {:?}\nsorted:   {:?}\nunsorted: {:?}",
            expected_sorted, sorted, unsorted
        );

        assert_eq!(sorted, expected_sorted);
        assert_eq!(unsorted, values);
    }
}

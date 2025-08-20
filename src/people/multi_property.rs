#![allow(dead_code)]
/*!
The `define_multi_property!` macro (defined in `property.rs`) takes a name
and a tuple of property tags and implements `SortByTag` for the tuple. It
defines a derived property (vio `define_derived_property`) with the provided
name having the type of tuples of values corresponding to the provided tags.

```rust
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

/// Orders the fields of the tuple `Self` according to the sorted order of the
/// fields of the tuple `Tag`. The `SortedTag` associated type is the tuple type of
/// `Tag` after sorting. The `ReorderedValue` associated type is the tuple type of
/// `Self` after reordering, according to the sorted order of the fields of `Tag`.
pub trait SortByTag<Tag> {
    type SortedTag;
    type ReorderedValue;

    /// Converts a `Self` value to a `Self::ReorderedValue` value.
    fn reorder_by_tag(self) -> Self::ReorderedValue;
    /// The inverse of `reorder_by_tag`. Note that this is an associated function, not a method.
    fn unreorder_by_tag(sorted: Self::ReorderedValue) -> Self;
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
    /*
    // The macro above generates the following:
    impl SortByTag<(TagC, TagA, TagB)> for (u8, &'static str, f64) {
      type SortedTag = (TagA, TagB, TagC);
      type ReorderedValue = (&'static str, f64, u8);
      fn reorder_by_tag(self) -> Self::ReorderedValue {
        let (t0, t1, t2) = self;
        (t1, t2, t0)
      }
      fn unreorder_by_tag(sorted: Self::ReorderedValue) -> Self {
        let (s0, s1, s2) = sorted;
        (s2, s0, s1)
      }
    }
    */

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

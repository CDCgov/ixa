//! Properties are the main way to store and access data about people.
//!
//! # Properties
//!
//! Properties are defined using the `define_person_property!` and
//! `define_derived_property!` macros.
//!
//! # Multi-properties
//!
//! The `define_multi_property!` macro (defined in `property.rs`) takes a name and a tuple
//! of property tags. It defines a derived property (via `define_derived_property`) with the
//! provided name having the type of tuples of values corresponding to the provided tags.
//!
//! ```rust,ignore
//! use ixa::people::{define_multi_property, define_person_property};
//!
//! define_person_property!(Name, &'static str);
//! define_person_property!(Age, u8);
//! define_person_property!(Weight, f64);
//!
//! define_multi_property!(Profile, (Name, Age, Weight));
//! ```
//!
//! The new derived property is not automatically indexed. You can index it just
//! like any other property:
//!
//! ```rust, ignore
//! context.index_property(Profile);
//! ```

use std::any::TypeId;
use std::fmt::Debug;

use serde::Serialize;

use crate::hashing::hash_serialized_128;
use crate::people::data::PersonPropertyHolder;
use crate::{Context, PersonId};

/// We factor this out and provide a blanket implementation for all types that
/// can be value types for properties. This makes it convenient to reference
/// `PersonPropertyValue` trait constraints.
pub trait PersonPropertyValue: Copy + Debug + PartialEq + Serialize {}
impl<T> PersonPropertyValue for T where T: Copy + Debug + PartialEq + Serialize {}

/// An individual characteristic or state related to a person, such as age or
/// disease status.
///
/// Person properties should be defined with the [`define_person_property!()`],
/// [`define_person_property_with_default!()`] and [`define_derived_property!()`]
/// macros.
pub trait PersonProperty: Copy + 'static {
    /// The type of the property's values.
    type Value: PersonPropertyValue;
    /// Some properties might store a transformed version of the value in the index. This is the
    /// type of the transformed value. For simple properties this will be the same as `Self::Value`.
    type CanonicalValue: PersonPropertyValue;

    #[must_use]
    fn is_derived() -> bool {
        false
    }

    #[must_use]
    fn is_required() -> bool {
        false
    }

    #[must_use]
    fn dependencies() -> Vec<Box<dyn PersonPropertyHolder>> {
        panic!("Dependencies not implemented");
    }

    fn register_dependencies(_: &Context) {
        panic!("Dependencies not implemented");
    }

    fn compute(context: &Context, person_id: PersonId) -> Self::Value;

    /// This transforms a `Self::Value` into a `Self::CanonicalValue`, e.g. for storage in an index.
    /// For simple properties, this is the identity function.
    #[must_use]
    fn make_canonical(value: Self::Value) -> Self::CanonicalValue;

    /// The inverse transform of `make_canonical`. For simple properties, this is the identity function.
    #[must_use]
    fn make_uncanonical(value: Self::CanonicalValue) -> Self::Value;
    fn get_instance() -> Self;
    fn name() -> &'static str;

    /// Returns a string representation of the property value, e.g. for writing to a CSV file.
    /// If `make_uncanonical` is nontrivial, this method usually transforms `value` into a
    /// `Self::Value` first so that the value is formatted in a way the user expects.
    #[must_use]
    fn get_display(value: &Self::CanonicalValue) -> String;

    /// For cases when the property's hash needs to be computed in a special way.
    #[must_use]
    fn hash_property_value(value: &Self::CanonicalValue) -> u128 {
        hash_serialized_128(value)
    }

    /// Overridden by multi-properties, which use the `TypeId` of the ordered tuple so that tuples
    /// with the same component types in a different order will have the same type ID.
    #[must_use]
    fn type_id() -> TypeId {
        TypeId::of::<Self>()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::people::{PeoplePlugin, Query};
    use crate::prelude::*;
    use crate::PersonProperty;

    define_person_property!(Pu32, u32);
    define_person_property!(POu32, Option<u32>);

    define_person_property!(Name, &'static str);
    define_person_property!(Age, u8);
    define_person_property!(Weight, f64);

    define_multi_property!(ProfileNAW, (Name, Age, Weight));
    define_multi_property!(ProfileAWN, (Age, Weight, Name));
    define_multi_property!(ProfileWAN, (Weight, Age, Name));

    #[test]
    fn test_multi_property_ordering() {
        let a: <ProfileNAW as PersonProperty>::Value = ("Jane", 22, 180.5);
        let b: <ProfileAWN as PersonProperty>::Value = (22, 180.5, "Jane");
        let c: <ProfileWAN as PersonProperty>::Value = (180.5, 22, "Jane");

        assert_eq!(ProfileNAW::type_id(), ProfileAWN::type_id());
        assert_eq!(ProfileNAW::type_id(), ProfileWAN::type_id());

        let a_canonical: <ProfileNAW as PersonProperty>::CanonicalValue =
            ProfileNAW::make_canonical(a);
        let b_canonical: <ProfileAWN as PersonProperty>::CanonicalValue =
            ProfileAWN::make_canonical(b);
        let c_canonical: <ProfileWAN as PersonProperty>::CanonicalValue =
            ProfileWAN::make_canonical(c);

        assert_eq!(a_canonical, b_canonical);
        assert_eq!(a_canonical, c_canonical);

        // Actually, all of the `Profile***::hash_property_value` methods should be the same,
        // so we could use any single one.
        assert_eq!(
            ProfileNAW::hash_property_value(&a_canonical),
            ProfileAWN::hash_property_value(&b_canonical)
        );
        assert_eq!(
            ProfileNAW::hash_property_value(&a_canonical),
            ProfileWAN::hash_property_value(&c_canonical)
        );

        // Since the canonical values are the same, we could have used any single one, but this
        // demonstrates that we can convert from one order to another.
        assert_eq!(ProfileNAW::make_uncanonical(b_canonical), a);
        assert_eq!(ProfileAWN::make_uncanonical(c_canonical), b);
        assert_eq!(ProfileWAN::make_uncanonical(a_canonical), c);
    }

    #[test]
    fn test_multi_property_vs_property_query() {
        let mut context = Context::new();

        context
            .add_person(((Name, "John"), (Age, 42), (Weight, 220.5)))
            .unwrap();
        context
            .add_person(((Name, "Jane"), (Age, 22), (Weight, 180.5)))
            .unwrap();
        context
            .add_person(((Name, "Bob"), (Age, 32), (Weight, 190.5)))
            .unwrap();
        context
            .add_person(((Name, "Alice"), (Age, 22), (Weight, 170.5)))
            .unwrap();

        context.index_property(ProfileNAW);

        {
            let data = context.get_data(PeoplePlugin);
            assert!(data
                .property_indexes
                .borrow()
                .get(&ProfileNAW::type_id())
                .is_some());
        }

        {
            let example_query = ((Name, "Alice"), (Age, 22), (Weight, 170.5));
            let query_multi_property_type_id = Query::multi_property_type_id(&example_query);
            assert!(query_multi_property_type_id.is_some());
            assert_eq!(ProfileNAW::type_id(), query_multi_property_type_id.unwrap());
            assert_eq!(
                Query::multi_property_value_hash(&example_query),
                ProfileNAW::hash_property_value(&ProfileNAW::make_canonical(("Alice", 22, 170.5)))
            );
        }

        context.with_query_results((ProfileNAW, ("John", 42, 220.5)), &mut |results| {
            assert_eq!(results.len(), 1);
        });
    }

    #[test]
    fn test_get_display() {
        let mut context = Context::new();
        let person = context.add_person(((POu32, Some(42)), (Pu32, 22))).unwrap();
        assert_eq!(
            format!(
                "{:}",
                POu32::get_display(&context.get_person_property(person, POu32))
            ),
            "42"
        );
        assert_eq!(
            format!(
                "{:}",
                Pu32::get_display(&context.get_person_property(person, Pu32))
            ),
            "22"
        );
        let person2 = context.add_person(((POu32, None), (Pu32, 11))).unwrap();
        assert_eq!(
            format!(
                "{:}",
                POu32::get_display(&context.get_person_property(person2, POu32))
            ),
            "None"
        );
    }

    #[test]
    fn test_debug_trait() {
        let property = Pu32;
        let debug_str = format!("{:?}", property);
        assert_eq!(debug_str, "Pu32");

        let property = POu32;
        let debug_str = format!("{:?}", property);
        assert_eq!(debug_str, "POu32");
    }
}

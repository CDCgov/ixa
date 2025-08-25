use crate::hashing::hash_serialized_128;
use crate::people::data::PersonPropertyHolder;
use crate::{Context, PersonId};
use serde::Serialize;
use std::any::TypeId;
use std::fmt::Debug;

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

#[macro_export]
macro_rules! __define_person_property_common {
    ($person_property:ident, $value:ty, $compute_fn:expr, $is_required:expr, $display_impl:expr) => {
        #[derive(Debug, Copy, Clone, Eq, PartialEq)]
        pub struct $person_property;
        impl $crate::people::PersonProperty for $person_property {
            type Value = $value;
            type CanonicalValue = $value;
            fn compute(
                _context: &$crate::context::Context,
                _person: $crate::people::PersonId,
            ) -> Self::CanonicalValue {
                $compute_fn(_context, _person)
            }
            fn make_canonical(value: Self::Value) -> Self::CanonicalValue {
                value
            }
            fn make_uncanonical(value: Self::CanonicalValue) -> Self::Value {
                value
            }
            fn is_required() -> bool {
                $is_required
            }
            fn get_instance() -> Self {
                $person_property
            }
            fn name() -> &'static str {
                stringify!($person_property)
            }
            fn get_display(value: &Self::CanonicalValue) -> String {
                $display_impl(value)
            }
        }
    };
}

/// Defines a person property with the following parameters:
/// * `$person_property`: A name for the identifier type of the property
/// * `$value`: The type of the property's value
/// * `$initialize`: (Optional) A function that takes a `Context` and `PersonId` and
///   returns the initial value. If it is not defined, calling `get_person_property`
///   on the property without explicitly setting a value first will panic.
#[macro_export]
macro_rules! define_person_property {
    // Option<T> with initializer
    ($person_property:ident, Option<$value:ty>, $initialize:expr) => {
        $crate::__define_person_property_common!(
            $person_property,
            Option<$value>,
            $initialize,
            false,
            |&value| {
                match value {
                    Some(v) => format!("{:?}", v),
                    None => "None".to_string(),
                }
            }
        );
    };
    // T with initializer
    ($person_property:ident, $value:ty, $initialize:expr) => {
        $crate::__define_person_property_common!(
            $person_property,
            $value,
            $initialize,
            false,
            |&value| format!("{:?}", value)
        );
    };
    // Option<T> without initializer
    ($person_property:ident, Option<$value:ty>) => {
        $crate::__define_person_property_common!(
            $person_property,
            Option<$value>,
            |_, _| panic!("Property not initialized when person created."),
            true,
            |&value| {
                match value {
                    Some(v) => format!("{:?}", v),
                    None => "None".to_string(),
                }
            }
        );
    };
    // T without initializer
    ($person_property:ident, $value:ty) => {
        $crate::__define_person_property_common!(
            $person_property,
            $value,
            |_, _| panic!("Property not initialized when person created."),
            true,
            |&value| format!("{:?}", value)
        );
    };
}
pub use define_person_property;

/// Defines a person property with the following parameters:
/// * `$person_property`: A name for the identifier type of the property
/// * `$value`: The type of the property's value
/// * `$default`: An initial value
#[macro_export]
macro_rules! define_person_property_with_default {
    ($person_property:ident, Option<$value:ty>, $default:expr) => {
        $crate::define_person_property!(
            $person_property,
            Option<$value>,
            |_context, _person_id| { $default }
        );
    };
    ($person_property:ident, $value:ty, $default:expr) => {
        $crate::define_person_property!($person_property, $value, |_context, _person_id| {
            $default
        });
    };
}
pub use define_person_property_with_default;

/// Defines a derived person property with the following parameters:
/// * `$person_property`: A name for the identifier type of the property
/// * `$value`: The type of the property's value
/// * `[$($dependency),+]`: A list of person properties the derived property depends on
/// * `[$($dependency),*]`: A list of global properties the derived property depends on (optional)
/// * `$calculate`: A closure that takes the values of each dependency and returns the derived value
/// * `$display`: A closure that takes the value of the derived property and returns a string representation
/// * `$hash_fn`: A function that can compute the hash of values of this property
#[macro_export]
macro_rules! __define_derived_property_common {
    (
        $derived_property:ident,
        $value:ty,
        $canonical_value:ty,
        $compute_canonical_impl:expr,
        $compute_uncanonical_impl:expr,
        $at_dependency_registration:expr,
        [$($dependency:ident),*],
        [$($global_dependency:ident),*],
        |$($param:ident),+| $derive_fn:expr,
        $display_impl:expr,
        $hash_fn:expr,
        $type_id_impl:expr
    ) => {
        #[derive(Debug, Copy, Clone)]
        pub struct $derived_property;

        impl $crate::people::PersonProperty for $derived_property {
            type Value = $value;
            type CanonicalValue = $canonical_value;

            fn compute(context: &$crate::context::Context, person_id: $crate::people::PersonId) -> Self::Value {
                #[allow(unused_imports)]
                use $crate::global_properties::ContextGlobalPropertiesExt;
                #[allow(unused_parens)]
                let ($($param,)*) = (
                    $(context.get_person_property(person_id, $dependency)),*,
                    $(
                        context.get_global_property_value($global_dependency)
                            .expect(&format!("Global property {} not initialized", stringify!($global_dependency)))
                    ),*
                );
                #[allow(non_snake_case)]
                (|$($param),+| $derive_fn)($($param),+)
            }
            fn make_canonical(value: Self::Value) -> Self::CanonicalValue {
                ($compute_canonical_impl)(value)
            }
            fn make_uncanonical(value: Self::CanonicalValue) -> Self::Value {
                ($compute_uncanonical_impl)(value)
            }
            fn is_derived() -> bool { true }
            fn dependencies() -> Vec<Box<dyn $crate::people::PersonPropertyHolder>> {
                vec![$(
                    Box::new($dependency) as Box<dyn $crate::people::PersonPropertyHolder>
                ),*]
            }
            fn register_dependencies(context: &$crate::context::Context) {
                $at_dependency_registration
                $(context.register_property::<$dependency>();)+
            }
            fn get_instance() -> Self {
                $derived_property
            }
            fn name() -> &'static str {
                stringify!($derived_property)
            }
            fn get_display(value: &Self::CanonicalValue) -> String {
                $display_impl(value)
            }
            fn hash_property_value(value: &Self::CanonicalValue) -> u128 {
                ($hash_fn)(value)
            }
            fn type_id() -> std::any::TypeId {
                $type_id_impl
            }
        }
    };
}

/// Defines a derived person property with the following parameters:
/// * `$person_property`: A name for the identifier type of the property
/// * `$value`: The type of the property's value
/// * `[$($dependency),+]`: A list of person properties the derived property depends on
/// * `[$($dependency),*]`: A list of global properties the derived property depends on (optional)
/// * $calculate: A closure that takes the values of each dependency and returns the derived value
#[macro_export]
macro_rules! define_derived_property {
    (
        $derived_property:ident,
        $value:ty,
        [$($dependency:ident),*],
        [$($global_dependency:ident),*],
        |$($param:ident),+| $derive_fn:expr
    ) => {
        $crate::__define_derived_property_common!(
            $derived_property,
            $value,
            $value,
            |v| v,
            |v| v,
            {/* empty*/},
            [$($dependency),*],
            [$($global_dependency),*],
            |$($param),+| $derive_fn,
            |&value| format!("{:?}", value),
            $crate::hashing::hash_serialized_128,
            std::any::TypeId::of::<Self>()
        );
    };

    // Empty global dependencies
    (
        $derived_property:ident,
        $value:ty,
        [$($dependency:ident),*],
        |$($param:ident),+| $derive_fn:expr
    ) => {
        $crate::__define_derived_property_common!(
            $derived_property,
            $value,
            $value,
            |v| v,
            |v| v,
            {/* empty*/},
            [$($dependency),*],
            [],
            |$($param),+| $derive_fn,
            |&value| format!("{:?}", value),
            $crate::hashing::hash_serialized_128,
            std::any::TypeId::of::<Self>()
        );
    };
}
pub use define_derived_property;

#[macro_export]
macro_rules! define_multi_property {
    (
        $person_property:ident,
        ( $($dependency:ident),+ )
    ) => {
        $crate::sorted_property_impl!(( $($dependency),+ ));
        $crate::paste::paste! {
            $crate::__define_derived_property_common!(
                // Name
                $person_property,

                // `PersonProperty::Value` type
                // <( $(<$dependency as $crate::people::PersonProperty>::Value),+ ) as $crate::people::SortByTag<( $($dependency),+ )>>,
                ( $(<$dependency as $crate::people::PersonProperty>::Value),+ ),

                // `PersonProperty::CanonicalValue` type
                <( $(<$dependency as $crate::people::PersonProperty>::Value),+ ) as $crate::people::SortByTag<( $($dependency),+ )>>::ReorderedValue,

                // Function to transform a `PersonProperty::Value` to a `PersonProperty::CanonicalValue`
                $crate::people::SortByTag::<( $($dependency),+ )>::reorder_by_tag,

                // Function to transform a `PersonProperty::CanonicalValue` to a `PersonProperty::Value`
                $crate::people::SortByTag::<( $($dependency),+ )>::unreorder_by_tag,

                // Code that runs at dependency registration time
                {
                    let type_ids = &mut [$($dependency::type_id()),+ ];
                    type_ids.sort();
                    $crate::people::register_type_ids_to_muli_property_id(type_ids, Self::type_id());
                },

                // Property dependency list
                [$($dependency),+],

                // Global property dependency list
                [],

                // A function that takes the values of each dependency and returns the derived value
                |$( [<_ $dependency:lower>] ),+| {
                    ( $( [<_ $dependency:lower>] ),+ )
                },

                // A function that takes a value and returns a string representation of it
                |values_tuple: &Self::CanonicalValue| {
                    let values_tuple: Self::Value = $crate::people::SortByTag::<( $($dependency),+ )>::unreorder_by_tag(*values_tuple);
                    let mut displayed = String::from("(");
                    let ( $( [<_ $dependency:lower>] ),+ ) = values_tuple;
                    $(
                        displayed.push_str(<$dependency as $crate::PersonProperty>::get_display(
                            & <$dependency as $crate::PersonProperty>::make_canonical([<_ $dependency:lower>])
                        ).as_str());
                        displayed.push_str(", ");
                    )+
                    displayed.truncate(displayed.len() - 2);
                    displayed.push_str(")");
                    displayed
                },

                // A function that computes the hash of a value of this property
                $crate::hashing::hash_serialized_128,

                // The Type ID of the property.
                // The type ID of a multi-property is the type ID of the SORTED tuple of its
                // components. This is so that tuples with the same component types in a different
                // order will have the same type ID.
                std::any::TypeId::of::<
                <( $(<$dependency as $crate::people::PersonProperty>::Value),+ )
                as $crate::people::SortByTag<( $($dependency),+ )>>::SortedTag>()
            );
        }
    };
}
pub use define_multi_property;

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

        let results = context.query_people((ProfileNAW, ("John", 42, 220.5)));
        assert_eq!(results.len(), 1);
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

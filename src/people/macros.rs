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
macro_rules! __define_derived_person_property_common {
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
macro_rules! define_derived_person_property {
    (
        $derived_property:ident,
        $value:ty,
        [$($dependency:ident),*],
        [$($global_dependency:ident),*],
        |$($param:ident),+| $derive_fn:expr
    ) => {
        $crate::__define_derived_person_property_common!(
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
        $crate::__define_derived_person_property_common!(
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
pub use define_derived_person_property;

/// Defines a named multi-property composed of a tuple of several existing other properties.
/// - `$person_property`: The name of the new multi-property type.
/// - `( $($dependency),+ )`: A non-empty, comma-separated, ordered list of existing
///   property identifiers that this multi-property is composed from.
#[macro_export]
macro_rules! define_multi_property {
    (
        $person_property:ident,
        ( $($dependency:ident),+ )
    ) => {
        // $crate::sorted_property_impl!(( $($dependency),+ ));
        $crate::paste::paste! {
            $crate::__define_derived_person_property_common!(
                // Name
                $person_property,

                // `PersonProperty::Value` type
                ( $(<$dependency as $crate::people::PersonProperty>::Value),+ ),

                // `PersonProperty::CanonicalValue` type
                $crate::sorted_value_type!(( $($dependency),+ )),

                // Function to transform a `PersonProperty::Value` to a `PersonProperty::CanonicalValue`
                $person_property::reorder_by_tag,

                // Function to transform a `PersonProperty::CanonicalValue` to a `PersonProperty::Value`
                $person_property::unreorder_by_tag,

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

                // A function that takes a canonical value and returns a string representation of it.
                |values_tuple: &Self::CanonicalValue| {
                    // ice tThe string representation uses the original (unsorted) ordering.
                    let values_tuple: Self::Value = Self::unreorder_by_tag(*values_tuple);
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
                std::any::TypeId::of::<$crate::sorted_tag!(( $($dependency),+ ))>()
            );
            $crate::impl_make_canonical!($person_property, ( $($dependency),+ ));
        }
    };
}
pub use define_multi_property;

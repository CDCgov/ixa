use crate::people::data::PersonPropertyHolder;
use crate::{Context, PersonId};
use serde::Serialize;
use std::fmt::Debug;

/// An individual characteristic or state related to a person, such as age or
/// disease status.
///
/// Person properties should be defined with the [`define_person_property!()`],
/// [`define_person_property_with_default!()`] and [`define_derived_property!()`]
/// macros.
pub trait PersonProperty: Copy {
    type Value: Copy + Debug + PartialEq + Serialize;
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
    fn compute(context: &Context, person_id: PersonId) -> Self::Value;
    fn get_instance() -> Self;
    fn name() -> &'static str;
}

/// Defines a person property with the following parameters:
/// * `$person_property`: A name for the identifier type of the property
/// * `$value`: The type of the property's value
/// * `$initialize`: (Optional) A function that takes a `Context` and `PersonId` and
///   returns the initial value. If it is not defined, calling `get_person_property`
///   on the property without explicitly setting a value first will panic.
#[macro_export]
macro_rules! define_person_property {
    ($person_property:ident, $value:ty, $initialize:expr) => {
        #[derive(Debug, Copy, Clone)]
        pub struct $person_property;
        impl $crate::people::PersonProperty for $person_property {
            type Value = $value;
            fn compute(
                _context: &$crate::context::Context,
                _person: $crate::people::PersonId,
            ) -> Self::Value {
                $initialize(_context, _person)
            }
            fn get_instance() -> Self {
                $person_property
            }
            fn name() -> &'static str {
                stringify!($person_property)
            }
        }
    };
    ($person_property:ident, $value:ty) => {
        #[derive(Debug, Copy, Clone)]
        pub struct $person_property;
        impl $crate::people::PersonProperty for $person_property {
            type Value = $value;
            fn compute(
                _context: &$crate::context::Context,
                _person: $crate::people::PersonId,
            ) -> Self::Value {
                panic!("Property not initialized when person created.");
            }
            fn is_required() -> bool {
                true
            }
            fn get_instance() -> Self {
                $person_property
            }
            fn name() -> &'static str {
                stringify!($person_property)
            }
        }
    };
}
pub use define_person_property;

/// Defines a person property with the following parameters:
/// * `$person_property`: A name for the identifier type of the property
/// * `$value`: The type of the property's value
/// * `$default`: An initial value
#[macro_export]
macro_rules! define_person_property_with_default {
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
        #[derive(Debug, Copy, Clone)]
        pub struct $derived_property;

        impl $crate::people::PersonProperty for $derived_property {
            type Value = $value;
            fn compute(context: &$crate::context::Context, person_id: $crate::people::PersonId) -> Self::Value {
                #[allow(unused_imports)]
                use $crate::global_properties::ContextGlobalPropertiesExt;
                #[allow(unused_parens)]
                let ($($param,)*) = (
                    $(context.get_person_property(person_id, $dependency)),*,
                    $(
                        *context.get_global_property_value($global_dependency)
                            .expect(&format!("Global property {} not initialized", stringify!($global_dependency)))
                    ),*
                );
                (|$($param),+| $derive_fn)($($param),+)
            }
            fn is_derived() -> bool { true }
            fn dependencies() -> Vec<Box<dyn $crate::people::PersonPropertyHolder>> {
                vec![$(Box::new($dependency)),+]
            }
            fn get_instance() -> Self {
                $derived_property
            }
            fn name() -> &'static str {
                stringify!($derived_property)
            }
        }
    };
    (
        $derived_property:ident,
        $value:ty,
        [$($dependency:ident),*],
        |$($param:ident),+| $derive_fn:expr
    ) => {
        define_derived_property!(
            $derived_property,
            $value,
            [$($dependency),*],
            [],
            |$($param),+| $derive_fn
        );
    };
}
pub use define_derived_property;

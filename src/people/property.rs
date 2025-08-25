use crate::people::data::PersonPropertyHolder;
use crate::{Context, PersonId};
use serde::Serialize;
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
    type Value: PersonPropertyValue;
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
    fn get_instance() -> Self;
    fn name() -> &'static str;
    fn get_display(value: &Self::Value) -> String;

    fn hash_property_value(value: &Self::Value) -> u128 {
        hash_serialized_128(value)
    }
}

#[macro_export]
macro_rules! __define_person_property_common {
    ($person_property:ident, $value:ty, $compute_fn:expr, $is_required:expr, $display_impl:expr) => {
        #[derive(Debug, Copy, Clone)]
        pub struct $person_property;
        impl $crate::people::PersonProperty for $person_property {
            type Value = $value;
            fn compute(
                _context: &$crate::context::Context,
                _person: $crate::people::PersonId,
            ) -> Self::Value {
                $compute_fn(_context, _person)
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
            fn get_display(value: &Self::Value) -> String {
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
            fn register_dependencies(context: &$crate::context::Context) {
                $(context.register_property::<$dependency>();)+
            }
            fn get_instance() -> Self {
                $derived_property
            }
            fn name() -> &'static str {
                stringify!($derived_property)
            }
            fn get_display(value: &Self::Value) -> String {
                format!("{:?}", value)
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
use crate::hashing::hash_serialized_128;
pub use define_derived_property;

#[macro_export]
macro_rules! define_multi_property_index {
    (
        $($dependency:ident),+
    ) => {
        $crate::paste::paste! {
            define_derived_property!(
                [< $($dependency)+ Query >],
                $crate::people::HashValueType,
                [$($dependency),+],
                |$([< $dependency:lower >]),+| {
                    let combined = vec!(
                        $(
                            (std::any::TypeId::of::<$dependency>(),
                            $dependency::hash_property_value(&[< $dependency:lower >]))
                        ),*
                    );
                    $crate::people::index::get_multi_property_value_hash(&combined)
                }
            );

            $crate::people::index::add_multi_property_index::<[< $($dependency)+ Query >]>(
                #[allow(clippy::useless_vec)]
                &mut vec![
                    $(
                        std::any::TypeId::of::<$dependency>(),
                    )*
                ],
                std::any::TypeId::of::<[< $($dependency)+ Query >]>(),
            );
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prelude::*;

    define_person_property!(Pu32, u32);
    define_person_property!(POu32, Option<u32>);

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
        let value = Pu32;
        let debug_str = format!("{:?}", value);
        // You can check for the struct name or any expected output
        assert!(debug_str.contains("Pu32"));
        let value = POu32;
        let debug_str = format!("{:?}", value);
        // You can check for the struct name or any expected output
        assert!(debug_str.contains("POu32"));
    }
}

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

#[macro_export]
macro_rules! define_multi_index_property {
    (
        $derived_property:ident,
        [$($dependency:ident),+],
        |$($param:ident),+|
    ) => {

        define_derived_property!($derived_property, IndexValue, [$($dependency),*],
            | $(
                $param
            ),+ | {
            let mut combined = vec!(
                $(
                    (std::any::TypeId::of::<$dependency>(),
                    IndexValue::compute(&$param))
                ),*
            );
            combined.sort_by(|a, b| a.0.cmp(&b.0));
            let values = combined.iter().map(|x| x.1).collect::<Vec<_>>();
            IndexValue::compute(&values)
        });
        use $crate::people::index::add_multi_property_index;
        add_multi_property_index(
            &vec![
                $(
                    std::any::TypeId::of::<$dependency>(),
                )*
            ],
            std::any::TypeId::of::<$derived_property>(),
        );
    };
}

#[macro_export]
macro_rules! define_xyz {
    ($derived_property:ident, [$d1:ident, $d2:ident])  => { define_multi_index_property!($derived_property, [$d1, $d2], |d1, d2|); };
    ($derived_property:ident, [$d1:ident, $d2:ident, $d3:ident])  => { define_multi_index_property!($derived_property, [$d1, $d2, $d3], |d1, d2, d3|); };
    ($derived_property:ident, [$d1:ident, $d2:ident, $d3:ident, $d4:ident])  => { define_multi_index_property!($derived_property, [$d1, $d2, $d3, $d4], |d1, d2, d3, d4|); };
    ($derived_property:ident, [$d1:ident, $d2:ident, $d3:ident, $d4:ident, $d5:ident])  => { define_multi_index_property!($derived_property, [$d1, $d2, $d3, $d4, $d5], |d1, d2, d3, d4, d5|); };
    ($derived_property:ident, [$d1:ident, $d2:ident, $d3:ident, $d4:ident, $d5:ident, $d6:ident])  => { define_multi_index_property!($derived_property, [$d1, $d2, $d3, $d4, $d5, $d6], |d1, d2, d3, d4, d5, d6|); };
    ($derived_property:ident, [$d1:ident, $d2:ident, $d3:ident, $d4:ident, $d5:ident, $d6:ident, $d7:ident])  => { define_multi_index_property!($derived_property, [$d1, $d2, $d3, $d4, $d5, $d6, $d7], |d1, d2, d3, d4, d5, d6, d7|); };
    ($derived_property:ident, [$d1:ident, $d2:ident, $d3:ident, $d4:ident, $d5:ident, $d6:ident, $d7:ident, $d8:ident])  => { define_multi_index_property!($derived_property, [$d1, $d2, $d3, $d4, $d5, $d6, $d7, $d8], |d1, d2, d3, d4, d5, d6, d7, d8|); };
    ($derived_property:ident, [$d1:ident, $d2:ident, $d3:ident, $d4:ident, $d5:ident, $d6:ident, $d7:ident, $d8:ident, $d9:ident])  => { define_multi_index_property!($derived_property, [$d1, $d2, $d3, $d4, $d5, $d6, $d7, $d8, $d9], |d1, d2, d3, d4, d5, d6, d7, d8, d9|); };
    ($derived_property:ident, [$d1:ident, $d2:ident, $d3:ident, $d4:ident, $d5:ident, $d6:ident, $d7:ident, $d8:ident, $d9:ident, $d10:ident])  => { define_multi_index_property!($derived_property, [$d1, $d2, $d3, $d4, $d5, $d6, $d7, $d8, $d9, $d10], |d1, d2, d3, d4, d5, d6, d7, d8, d9, d10|); };
    ($derived_property:ident, [$d1:ident, $d2:ident, $d3:ident, $d4:ident, $d5:ident, $d6:ident, $d7:ident, $d8:ident, $d9:ident, $d10:ident, $d11:ident])  => { define_multi_index_property!($derived_property, [$d1, $d2, $d3, $d4, $d5, $d6, $d7, $d8, $d9, $d10, $d11], |d1, d2, d3, d4, d5, d6, d7, d8, d9, d10, d11|); };
    ($derived_property:ident, [$d1:ident, $d2:ident, $d3:ident, $d4:ident, $d5:ident, $d6:ident, $d7:ident, $d8:ident, $d9:ident, $d10:ident, $d11:ident, $d12:ident])  => { define_multi_index_property!($derived_property, [$d1, $d2, $d3, $d4, $d5, $d6, $d7, $d8, $d9, $d10, $d11, $d12], |d1, d2, d3, d4, d5, d6, d7, d8, d9, d10, d11, d12|); };
    ($derived_property:ident, [$d1:ident, $d2:ident, $d3:ident, $d4:ident, $d5:ident, $d6:ident, $d7:ident, $d8:ident, $d9:ident, $d10:ident, $d11:ident, $d12:ident, $d13:ident])  => { define_multi_index_property!($derived_property, [$d1, $d2, $d3, $d4, $d5, $d6, $d7, $d8, $d9, $d10, $d11, $d12, $d13], |d1, d2, d3, d4, d5, d6, d7, d8, d9, d10, d11, d12, d13|); };
    ($derived_property:ident, [$d1:ident, $d2:ident, $d3:ident, $d4:ident, $d5:ident, $d6:ident, $d7:ident, $d8:ident, $d9:ident, $d10:ident, $d11:ident, $d12:ident, $d13:ident, $d14:ident])  => { define_multi_index_property!($derived_property, [$d1, $d2, $d3, $d4, $d5, $d6, $d7, $d8, $d9, $d10, $d11, $d12, $d13, $d14], |d1, d2, d3, d4, d5, d6, d7, d8, d9, d10, d11, d12, d13, d14|); };
    ($derived_property:ident, [$d1:ident, $d2:ident, $d3:ident, $d4:ident, $d5:ident, $d6:ident, $d7:ident, $d8:ident, $d9:ident, $d10:ident, $d11:ident, $d12:ident, $d13:ident, $d14:ident, $d15:ident])  => { define_multi_index_property!($derived_property, [$d1, $d2, $d3, $d4, $d5, $d6, $d7, $d8, $d9, $d10, $d11, $d12, $d13, $d14, $d15] |d1, d2, d3, d4, d5, d6, d7, d8, d9, d10, d11, d12, d13, d14, d15|); };
    ($derived_property:ident, [$d1:ident, $d2:ident, $d3:ident, $d4:ident, $d5:ident, $d6:ident, $d7:ident, $d8:ident, $d9:ident, $d10:ident, $d11:ident, $d12:ident, $d13:ident, $d14:ident, $d15:ident, $d16:ident])  => { define_multi_index_property!($derived_property, [$d1, $d2, $d3, $d4, $d5, $d6, $d7, $d8, $d9, $d10, $d11, $d12, $d13, $d14, $d15, $d16] |d1, d2, d3, d4, d5, d6, d7, d8, d9, d10, d11, d12, d13, d14, d15, d16|); };
    ($derived_property:ident, [$d1:ident, $d2:ident, $d3:ident, $d4:ident, $d5:ident, $d6:ident, $d7:ident, $d8:ident, $d9:ident, $d10:ident, $d11:ident, $d12:ident, $d13:ident, $d14:ident, $d15:ident, $d16:ident, $d17:ident])  => { define_multi_index_property!($derived_property, [$d1, $d2, $d3, $d4, $d5, $d6, $d7, $d8, $d9, $d10, $d11, $d12, $d13, $d14, $d15, $d16, $d17] |d1, d2, d3, d4, d5, d6, d7, d8, d9, d10, d11, d12, d13, d14, d15, d16, d17|); };
    ($derived_property:ident, [$d1:ident, $d2:ident, $d3:ident, $d4:ident, $d5:ident, $d6:ident, $d7:ident, $d8:ident, $d9:ident, $d10:ident, $d11:ident, $d12:ident, $d13:ident, $d14:ident, $d15:ident, $d16:ident, $d17:ident, $d18:ident])  => { define_multi_index_property!($derived_property, [$d1, $d2, $d3, $d4, $d5, $d6, $d7, $d8, $d9, $d10, $d11, $d12, $d13, $d14, $d15, $d16, $d17, $d18] |d1, d2, d3, d4, d5, d6, d7, d8, d9, d10, d11, d12, d13, d14, d15, d16, d17, d18|); };
    ($derived_property:ident, [$d1:ident, $d2:ident, $d3:ident, $d4:ident, $d5:ident, $d6:ident, $d7:ident, $d8:ident, $d9:ident, $d10:ident, $d11:ident, $d12:ident, $d13:ident, $d14:ident, $d15:ident, $d16:ident, $d17:ident, $d18:ident, $d19:ident])  => { define_multi_index_property!($derived_property, [$d1, $d2, $d3, $d4, $d5, $d6, $d7, $d8, $d9, $d10, $d11, $d12, $d13, $d14, $d15, $d16, $d17, $d18] |d1, d2, d3, d4, d5, d6, d7, d8, d9, d10, d11, d12, d13, d14, d15, d16, d17, d18, d19|); };
    ($derived_property:ident, [$d1:ident, $d2:ident, $d3:ident, $d4:ident, $d5:ident, $d6:ident, $d7:ident, $d8:ident, $d9:ident, $d10:ident, $d11:ident, $d12:ident, $d13:ident, $d14:ident, $d15:ident, $d16:ident, $d17:ident, $d18:ident, $d19:ident, $d20:ident])  => { define_multi_index_property!($derived_property, [$d1, $d2, $d3, $d4, $d5, $d6, $d7, $d8, $d9, $d10, $d11, $d12, $d13, $d14, $d15, $d16, $d17, $d18, $d19, $d20] |d1, d2, d3, d4, d5, d6, d7, d8, d9, d10, d11, d12, d13, d14, d15, d16, d17, d18, d19, d20|); };
}
/*
use seq_macro::seq;


seq!(N in 1..=10 {
    #[macro_export]
    macro_rules! define_xyz {
        ( $derived_property:ident, $( $param:expr ),{N} ) => {
            $func($( $param ),*);
        };
    }
});

#[macro_export]
macro_rules! define_xyz {
    ($derived_property:ident, [$($d:ident),*]) => {
        seq!(N in 1..=20 {
            match ($($d),*) {
                ($d~N),*) => {
                    define_multi_index_property!(
                        $derived_property,
                        [$( $d ),*],
                        |$( $d ),*|
                    );
                }
            }
}
*/

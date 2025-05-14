use crate::people::data::PersonPropertyHolder;
use crate::people::PeoplePlugin;
use crate::{Context, PersonId};
use serde::Serialize;
use std::fmt::Debug;

/// An individual characteristic or state related to a person, such as age or
/// disease status.
///
/// Person properties should be defined with the [`define_person_property!()`],
/// [`define_person_property_with_default!()`] and [`define_derived_property!()`]
/// macros.
pub trait PersonProperty: Copy + 'static {
    type Value: Copy + Debug + PartialEq + Serialize;
    #[must_use]
    fn is_derived() -> bool {
        false
    }

    #[must_use]
    fn name() -> &'static str;

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

    fn get_instance() -> Self;

    /// Computes the value of the property for the given person. For nonderived properties, this is just a lookup.
    /// The default behavior is to assign an initial value to the entity's property if the lookup fails to return
    /// a value.
    fn compute(context: &Context, person_id: PersonId) -> Option<Self::Value> {
        let value = context
            .get_data_container(PeoplePlugin)
            .unwrap()
            .get_person_property_ref(person_id, Self::get_instance());

        // Initialize the property. This does not fire a change event
        if value.is_none() {
            match Self::initial_value(context, person_id) {
                None => None,

                Some(value) => {
                    // Insert the initial value for the person_id
                    // ToDo(Robert): This unwrap is temporary until we remove RefCell.
                    let data_container = context.get_data_container(PeoplePlugin).unwrap();
                    data_container.set_person_property(person_id, Self::get_instance(), value);
                    Some(value)
                }
            }
        } else {
            *value
        }
    }

    /// Overload to compute an initial value for a value that has not been set. The default behavior of
    /// `Property::compute` is to assign this initial value to the entity.
    #[must_use]
    fn initial_value(_context: &Context, _entity_id: PersonId) -> Option<Self::Value> {
        None
    }
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

            fn get_instance() -> Self {
                $person_property
            }

            fn name() -> &'static str {
                stringify!($person_property)
            }

            fn initial_value(
                context: &$crate::context::Context,
                person_id: $crate::people::PersonId,
            ) -> Option<Self::Value> {
                Some($initialize(context, person_id))
            }
        }
    };
    ($person_property:ident, $value:ty) => {
        #[derive(Debug, Copy, Clone)]
        pub struct $person_property;
        impl $crate::people::PersonProperty for $person_property {
            type Value = $value;

            fn is_required() -> bool {
                true
            }

            fn name() -> &'static str {
                stringify!($person_property)
            }

            fn get_instance() -> Self {
                $person_property
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

            fn compute(context: &$crate::context::Context, person_id: $crate::people::PersonId) -> Option<Self::Value> {
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
                Some((|$($param),+| $derive_fn)($($param),+))
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
macro_rules! define_multi_property_index {
    (
        $($dependency:ident),+
    ) => {
        $crate::paste::paste! {
            define_derived_property!(
                [< $($dependency)+ Query >],
                $crate::people::index::IndexValue,
                [$($dependency),+],
                |$([< $dependency:lower >]),+| {
                    let mut combined = vec!(
                        $(
                            (std::any::TypeId::of::<$dependency>(),
                            $crate::people::index::IndexValue::compute(&[< $dependency:lower >]))
                        ),*
                    );
                    combined.sort_by(|a, b| a.0.cmp(&b.0));
                    let values = combined.iter().map(|x| x.1).collect::<Vec<_>>();
                    $crate::people::index::IndexValue::compute(&values)
                }
            );

            $crate::people::index::add_multi_property_index::<[< $($dependency)+ Query >]>(
                #[allow(clippy::useless_vec)]
                &vec![
                    $(
                        std::any::TypeId::of::<$dependency>(),
                    )*
                ],
                std::any::TypeId::of::<[< $($dependency)+ Query >]>(),
            );
        }
    };
}

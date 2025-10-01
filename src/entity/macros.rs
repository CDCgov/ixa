#[macro_export]
macro_rules! default_property_for {
    ($entity:ident, $property:ident $(< $($generic:ty),+ >)?, $default:expr) => {
        $crate::property_for!(
            $entity,
            $property $(< $($generic),+ >)?,
            |_| $default
        );
    };
}

#[macro_export]
macro_rules! property_for {
    // Default initializer
    ($entity:ident, $property:ident $(< $($generic:ty),+ >)?) => {
        $crate::property_for!(
            $entity,
            $property $(< $($generic),+ >)?,
            |_| panic!("Expected to be initialized")
        );
    };

    // Custom initializer
    ($entity:ident, $property:ident $(< $($generic:ty),+ >)?, $init_fn:expr) => {
        $crate::paste::paste! {
            $crate::property_for!(
                @impl
                $entity,
                $property $(< $($generic),+ >)?,
                ( $entity $property $($($generic)+)? ),
                $init_fn
            );
        }
    };

    (@impl $entity:ident, $prop_ty:ty, ($($marker_type:tt)+), $init_fn:expr) => {
        $crate::paste::paste! {
            pub struct [< $($marker_type)+ >];

            const _: () = {
                use $crate::entity::EntityProperty;
                use $crate::vec_cell::VecCell;

                type __Value = <$prop_ty as $crate::entity::Property>::Value;

                $crate::type_index!(
                    [< $($marker_type)+ >],
                    VecCell<Option<__Value>>,
                    EntityProperty
                );

                impl $crate::entity::PropertyFor<$entity> for $prop_ty {
                    fn type_index() -> usize {
                        <[< $($marker_type)+ >] as $crate::type_store::TypeIndex>::type_index()
                    }
                    fn initializer(context: &$crate::context::Context) -> Self::Value {
                        $init_fn(context)
                    }
                }
            };
        }
    };
}

#[macro_export]
macro_rules! define_entity {
    ($(#[$meta:meta])* $vis:vis struct $entity:ident) => {
        $(#[$meta])*
        #[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
        $vis struct $entity;

        $crate::type_index!($entity, $crate::entity::EntityData, $crate::entity::EntityMarker);

        impl $crate::entity::Entity for $entity {}
    };
}

#[macro_export]
macro_rules! define_entity_property {
    ($(#[$meta:meta])* $vis:vis struct $property:ident : $value:ty) => {
        $(#[$meta])*
        #[derive(Debug, Copy, Clone, PartialEq, Eq)]
        $vis struct $property;

        impl $crate::entity::Property for $property {
            type Value = $value;
        }
    };
    ($(#[$meta:meta])* $vis:vis enum $property:ident { $($variant:ident),* $(,)? } ) => {
        $(#[$meta])*
        #[derive(Debug, Copy, Clone, PartialEq, Eq)]
        $vis enum $property {
            $($variant),*
        }

        impl $crate::entity::Property for $property {
            type Value = $property;
        }
    };
}

#[macro_export]
macro_rules! entity_property_for {
    ($entity:ident => $property:ident) => {
        $crate::property_for!($entity, $property, |_context| panic!(
            "Expected property `{}` to be initialized before use",
            stringify!($property)
        ));
    };
    ($entity:ident => $property:ident, default = $default:expr) => {
        $crate::property_for!($entity, $property, |_context| $default);
    };
    ($entity:ident => $property:ident, init = $init:expr) => {
        $crate::property_for!($entity, $property, $init);
    };
}

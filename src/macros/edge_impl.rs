/*!

Macros for implementing edge types.

*/

#[macro_export]
macro_rules! define_edge_type {
    // Struct (tuple) and ZST
    (
        struct $name:ident $( ( $($visibility:vis $field_ty:ty),* $(,)? ) )?,
        $entity:ident
        $(, $($extra:tt)+),*
    ) => {
        #[derive(Debug, PartialEq, Clone)]
        pub struct $name $( ($($visibility $field_ty),*) )?;
        $crate::impl_edge_type!($name, $entity $(, $($extra)+)*);
    };

    // Struct (named fields)
    (
        struct $name:ident { $($visibility:vis $field_name:ident : $field_ty:ty),* $(,)? },
        $entity:ident
        $(, $($extra:tt)+),*
    ) => {
        #[derive(Copy, Debug, PartialEq)]
        pub struct $name { $($visibility $field_name : $field_ty),* }
        $crate::impl_edge_type!($name, $entity $(, $($extra)+)*);
    };

    // Enum
    (
        enum $name:ident {
            $($variant:ident),* $(,)?
        },
        $entity:ident
        $(, $($extra:tt)+),*
    ) => {
        #[derive(Copy, Debug, PartialEq)]
        pub enum $name {
            $($variant),*
        }
        $crate::impl_edge_type!($name, $entity $(, $($extra)+)*);
    };
}

#[macro_export]
macro_rules! impl_edge_type {
    (
        $edge_type:ident,
        $entity:ident $(,)?
    ) => {
        impl $crate::network::edge::EdgeType<$entity> for $edge_type {
            fn id() -> usize {
                // This static must be initialized with a compile-time constant expression.
                // We use `usize::MAX` as a sentinel to mean "uninitialized". This
                // static variable is shared among all instances of this concrete item type.
                static INDEX: std::sync::atomic::AtomicUsize =
                    std::sync::atomic::AtomicUsize::new(usize::MAX);

                // Fast path: already initialized.
                let index = INDEX.load(std::sync::atomic::Ordering::Relaxed);
                if index != usize::MAX {
                    return index;
                }

                // Slow path: initialize it.
                $crate::network::edge::initialize_edge_type_id::<$entity>(&INDEX)
            }
        }

        $crate::paste::paste! {
            $crate::ctor::declarative::ctor!{
                #[ctor]
                fn [<_register_edge_type_ $edge_type:snake _for_ $entity:snake>]() {
                    $crate::network::edge::add_to_edge_type_to_registry::<$entity, $edge_type>();
                }
            }
        }
    };
}

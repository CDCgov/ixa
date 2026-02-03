//! Macros to correctly define and implement the `Entity` trait.

/// Defines a zero-sized struct with the right derived traits and implements the `Entity` trait. If you already
/// have a type defined (struct, enum, etc.), you can use the `impl_entity!` macro instead.
#[macro_export]
macro_rules! define_entity {
    ($entity_name:ident) => {
        #[allow(unused)]
        #[derive(Default, Debug, PartialEq, Eq, Clone, Copy)]
        pub struct $entity_name;

        impl $entity_name {
            #[allow(unused)]
            pub fn new() -> Self {
                Self::default()
            }
        }

        $crate::impl_entity!($entity_name);
    };
}
pub use define_entity;

/// Implements the `Entity` trait for the given existing type and defines a type alias
/// of the form `MyEntityId = EntityId<MyEntity>`. For simple zero-sized types, use the
/// `define_entity!` macro instead, which will define the struct and derive all the super traits.
///
/// This macro ensures the correct implementation of the `Entity` trait. The tricky bit is the implementation of
/// `Entity::index`, which requires synchronization in multithreaded runtimes. This is an instance of
/// _correctness via macro_.
#[macro_export]
macro_rules! impl_entity {
    ($entity_name:ident) => {
        // Alias of the form `MyEntityId = EntityId<MyEntity>`
        $crate::paste::paste! {
            #[allow(unused)]
            pub type [<$entity_name Id>] = $crate::entity::EntityId<$entity_name>;
        }

        impl $crate::entity::Entity for $entity_name {
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
                $crate::entity::entity_store::initialize_entity_index(&INDEX)
            }

            fn as_any(&self) -> &dyn std::any::Any {
                self
            }
            fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
                self
            }
        }

        // Using `ctor` to initialize entities at program start-up means we know how many entities
        // there are at the time any `EntityStore` is created, which means we never have
        // to mutate `EntityStore` to initialize an `Entity` that hasn't yet been accessed.
        // (The mutation happens inside of a `OnceCell`, which we can already have ready
        // when we construct `EntityStore`.) In other words, we could do away with `ctor`
        // if we were willing to have a mechanism for interior mutability for `EntityStore`.
        $crate::paste::paste! {
            $crate::ctor::declarative::ctor!{
                #[ctor]
                fn [<_register_entity_$entity_name:snake>]() {
                    $crate::entity::entity_store::add_to_entity_registry::<$entity_name>();
                }
            }
        }

        // Generate entity-specific query macro (e.g., `person!` for `Person` entity)
        $crate::paste::paste! {
            /// Creates a query for this entity type.
            ///
            /// # Examples
            /// ```ignore
            #[doc = concat!("// Empty query (matches all ", stringify!($entity_name), " entities)")]
            #[doc = concat!("context.with_query_results(", stringify!([<$entity_name:snake>]), "![], ...)")]
            ///
            #[doc = concat!("// Single property query")]
            #[doc = concat!("context.with_query_results(", stringify!([<$entity_name:snake>]), "![Age(42)], ...)")]
            ///
            #[doc = concat!("// Multi-property query")]
            #[doc = concat!("context.with_query_results(", stringify!([<$entity_name:snake>]), "![Age(42), Name(\"Alice\")], ...)")]
            /// ```
            macro_rules! [<$entity_name:snake>] {
                () => {
                    $crate::entity::query::PropertyEntityValues0::<$entity_name>::new()
                };
                ($p0:expr) => {
                    $crate::entity::query::PropertyEntityValues1::<$entity_name, _>::new($p0)
                };
                ($p0:expr, $p1:expr) => {
                    $crate::entity::query::PropertyEntityValues2::<$entity_name, _, _>::new($p0, $p1)
                };
                ($p0:expr, $p1:expr, $p2:expr) => {
                    $crate::entity::query::PropertyEntityValues3::<$entity_name, _, _, _>::new($p0, $p1, $p2)
                };
                ($p0:expr, $p1:expr, $p2:expr, $p3:expr) => {
                    $crate::entity::query::PropertyEntityValues4::<$entity_name, _, _, _, _>::new($p0, $p1, $p2, $p3)
                };
                ($p0:expr, $p1:expr, $p2:expr, $p3:expr, $p4:expr) => {
                    $crate::entity::query::PropertyEntityValues5::<$entity_name, _, _, _, _, _>::new($p0, $p1, $p2, $p3, $p4)
                };
                ($p0:expr, $p1:expr, $p2:expr, $p3:expr, $p4:expr, $p5:expr) => {
                    $crate::entity::query::PropertyEntityValues6::<$entity_name, _, _, _, _, _, _>::new($p0, $p1, $p2, $p3, $p4, $p5)
                };
                ($p0:expr, $p1:expr, $p2:expr, $p3:expr, $p4:expr, $p5:expr, $p6:expr) => {
                    $crate::entity::query::PropertyEntityValues7::<$entity_name, _, _, _, _, _, _, _>::new($p0, $p1, $p2, $p3, $p4, $p5, $p6)
                };
                ($p0:expr, $p1:expr, $p2:expr, $p3:expr, $p4:expr, $p5:expr, $p6:expr, $p7:expr) => {
                    $crate::entity::query::PropertyEntityValues8::<$entity_name, _, _, _, _, _, _, _, _>::new($p0, $p1, $p2, $p3, $p4, $p5, $p6, $p7)
                };
                ($p0:expr, $p1:expr, $p2:expr, $p3:expr, $p4:expr, $p5:expr, $p6:expr, $p7:expr, $p8:expr) => {
                    $crate::entity::query::PropertyEntityValues9::<$entity_name, _, _, _, _, _, _, _, _, _>::new($p0, $p1, $p2, $p3, $p4, $p5, $p6, $p7, $p8)
                };
                ($p0:expr, $p1:expr, $p2:expr, $p3:expr, $p4:expr, $p5:expr, $p6:expr, $p7:expr, $p8:expr, $p9:expr) => {
                    $crate::entity::query::PropertyEntityValues10::<$entity_name, _, _, _, _, _, _, _, _, _, _>::new($p0, $p1, $p2, $p3, $p4, $p5, $p6, $p7, $p8, $p9)
                };
            }
            #[allow(unused)]
            pub(crate) use [<$entity_name:snake>];
        }
    };
}
pub use impl_entity;

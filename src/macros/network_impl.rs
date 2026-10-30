/*!

Macros for defining networks.

*/

/// Defines a network over an entity type.
///
/// With `layers = { .. }` the network is an enum with one variant per layer; without it,
/// a unit struct with a single layer. `data = T` sets the per-edge data type (default `()`).
///
/// ```
/// use ixa::prelude::*;
///
/// define_entity!(Person);
/// define_network!(Contact, Person, layers = { Household, School }, data = f64);
/// define_network!(InfectionTree, Person);
///
/// assert_eq!(Contact::ALL, &[Contact::Household, Contact::School]);
/// assert_eq!(InfectionTree.layer(), 0);
/// ```
#[macro_export]
macro_rules! define_network {
    (
        $name:ident, $entity:ident,
        layers = { $($layer:ident),+ $(,)? }
        $(, data = $data:ty)? $(,)?
    ) => {
        #[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
        pub enum $name { $($layer),+ }
        $crate::impl_network!($name, $entity, layers = [$($name::$layer),+] $(, data = $data)?);
    };
    (
        $name:ident, $entity:ident
        $(, data = $data:ty)? $(,)?
    ) => {
        #[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
        pub struct $name;
        $crate::impl_network!($name, $entity, layers = [$name] $(, data = $data)?);
    };
}

/// Implements [`Network`](crate::network::Network) for an existing type. `layers` must
/// list every value of the type, in layer order.
#[macro_export]
macro_rules! impl_network {
    (
        $name:ident, $entity:ident,
        layers = [ $($layer:path),+ $(,)? ] $(,)?
    ) => {
        $crate::impl_network!($name, $entity, layers = [$($layer),+], data = ());
    };
    (
        $name:ident, $entity:ident,
        layers = [ $($layer:path),+ $(,)? ],
        data = $data:ty $(,)?
    ) => {
        impl $crate::network::Network for $name {
            type Entity = $entity;
            type Data = $data;

            const ALL: &'static [Self] = &[$($layer),+];

            fn layer(self) -> usize {
                $crate::__network_layer_match!(self, 0usize; []; $($layer),+)
            }

            fn id() -> usize {
                // `usize::MAX` means "uninitialized". The static is shared by every
                // instance of this concrete type.
                static INDEX: std::sync::atomic::AtomicUsize =
                    std::sync::atomic::AtomicUsize::new(usize::MAX);

                let index = INDEX.load(std::sync::atomic::Ordering::Relaxed);
                if index != usize::MAX {
                    return index;
                }
                $crate::network::initialize_network_id::<$entity>(&INDEX)
            }
        }

        $crate::paste::paste! {
            $crate::ctor::declarative::ctor!{
                #[ctor(unsafe)]
                fn [<_register_network_ $name:snake _for_ $entity:snake>]() {
                    $crate::network::register_network::<$name>();
                }
            }
        }
    };
}

/// Expands to a `match` mapping each listed layer to its position.
#[doc(hidden)]
#[macro_export]
macro_rules! __network_layer_match {
    ($self:expr, $idx:expr; [$($arms:tt)*]; ) => {
        match $self { $($arms)* }
    };
    ($self:expr, $idx:expr; [$($arms:tt)*]; $head:path $(, $tail:path)*) => {
        $crate::__network_layer_match!($self, $idx + 1; [$($arms)* $head => $idx,]; $($tail),*)
    };
}

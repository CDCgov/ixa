/*!

Macros for defining networks.

*/

/// Defines a network over an entity type.
///
/// With `layers = { .. }` the network is an enum with one variant per layer; without it,
/// a unit struct with a single layer. `data = T` sets the per-edge data type (default `()`).
/// `validate = |data: &T| -> bool` rejects edge data at insertion; without it, `f64` and
/// `f32` data must be finite and non-negative and other types are stored as given.
/// `undirected` makes the network undirected: `add_edge`, `add_edges`, `remove_edge`,
/// `update_edge`, and `add_or_update_edge` always act on both directions, and edge counts
/// and whole-layer iteration are per pair. `data`, `validate`, and `undirected` may appear
/// in any order after `layers`.
///
/// ```
/// use ixa::prelude::*;
///
/// define_entity!(Person);
/// define_network!(Contact, Person, layers = { Household, School }, data = f64);
/// define_network!(InfectionTree, Person);
/// define_network!(Partnership, Person, layers = { Main, Casual }, undirected);
/// define_network!(Shared, Person, data = f64, validate = |rate: &f64| (0.0..=1.0).contains(rate));
///
/// assert_eq!(Contact::ALL, &[Contact::Household, Contact::School]);
/// assert!(!Contact::UNDIRECTED);
/// assert!(Partnership::UNDIRECTED);
/// assert_eq!(InfectionTree.layer(), 0);
/// assert!(Shared::validate(&0.5).is_ok());
/// assert!(Shared::validate(&1.5).is_err());
/// ```
#[macro_export]
macro_rules! define_network {
    (
        $name:ident, $entity:ident,
        layers = { $($layer:ident),+ $(,)? }
        $(, $($rest:tt)*)?
    ) => {
        #[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
        pub enum $name { $($layer),+ }
        $crate::impl_network!(
            $name, $entity, layers = [$($name::$layer),+]
            $(, $($rest)*)?
        );
    };
    (
        $name:ident, $entity:ident
        $(, $($rest:tt)*)?
    ) => {
        #[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
        pub struct $name;
        $crate::impl_network!(
            $name, $entity, layers = [$name]
            $(, $($rest)*)?
        );
    };
}

/// Implements [`Network`](crate::network::Network) for an existing type. `layers` must
/// list every value of the type, in layer order. `data`, `validate`, and `undirected` are
/// as in [`define_network!`](crate::define_network).
///
/// A repeated layer is a compile error:
///
/// ```compile_fail
/// use ixa::prelude::*;
///
/// define_entity!(Person);
/// #[derive(Copy, Clone, Debug, PartialEq, Eq)]
/// enum Setting { Home, School }
/// impl_network!(Setting, Person, layers = [Setting::Home, Setting::Home, Setting::School]);
/// ```
#[macro_export]
macro_rules! impl_network {
    (
        $name:ident, $entity:ident,
        layers = [ $($layer:path),+ $(,)? ]
        $(, $($rest:tt)*)?
    ) => {
        $crate::__impl_network_opts!(
            name = $name,
            entity = $entity,
            layers = [$($layer),+],
            data = (),
            has_data = false,
            validate = [],
            undirected = false;
            $($($rest)*)?
        );
    };
}

/// Parses optional `data`, `validate`, and `undirected` arguments in any order (at most
/// once each) and emits the [`Network`](crate::network::Network) impl and registration.
#[doc(hidden)]
#[macro_export]
macro_rules! __impl_network_opts {
    (
        name = $name:ident,
        entity = $entity:ident,
        layers = [$($layer:path),+],
        data = $old_data:ty,
        has_data = false,
        validate = [$($validate:expr)?],
        undirected = $undirected:tt;
        data = $new_data:ty $(, $($rest:tt)*)?
    ) => {
        $crate::__impl_network_opts!(
            name = $name,
            entity = $entity,
            layers = [$($layer),+],
            data = $new_data,
            has_data = true,
            validate = [$($validate)?],
            undirected = $undirected;
            $($($rest)*)?
        );
    };
    (
        name = $name:ident,
        entity = $entity:ident,
        layers = [$($layer:path),+],
        data = $data:ty,
        has_data = $has_data:tt,
        validate = [],
        undirected = $undirected:tt;
        validate = $new_validate:expr $(, $($rest:tt)*)?
    ) => {
        $crate::__impl_network_opts!(
            name = $name,
            entity = $entity,
            layers = [$($layer),+],
            data = $data,
            has_data = $has_data,
            validate = [$new_validate],
            undirected = $undirected;
            $($($rest)*)?
        );
    };
    (
        name = $name:ident,
        entity = $entity:ident,
        layers = [$($layer:path),+],
        data = $data:ty,
        has_data = $has_data:tt,
        validate = [$($validate:expr)?],
        undirected = false;
        undirected $(, $($rest:tt)*)?
    ) => {
        $crate::__impl_network_opts!(
            name = $name,
            entity = $entity,
            layers = [$($layer),+],
            data = $data,
            has_data = $has_data,
            validate = [$($validate)?],
            undirected = true;
            $($($rest)*)?
        );
    };
    (
        name = $name:ident,
        entity = $entity:ident,
        layers = [$($layer:path),+],
        data = $data:ty,
        has_data = $has_data:tt,
        validate = [$($validate:expr)?],
        undirected = $undirected:tt;
    ) => {
        impl $crate::network::Network for $name {
            type Entity = $entity;
            type Data = $data;

            const ALL: &'static [Self] = &[$($layer),+];
            const UNDIRECTED: bool = $undirected;

            fn layer(self) -> usize {
                $crate::__network_layer_match!(self, 0usize; []; $($layer),+)
            }

            fn validate(data: &Self::Data) -> Result<(), $crate::IxaError> {
                let valid: bool = $crate::__network_validate!(data $(, $validate)?);
                if valid {
                    Ok(())
                } else {
                    Err($crate::IxaError::InvalidEdgeData {
                        network: <Self as $crate::network::Network>::name().to_string(),
                        data: format!("{data:?}"),
                    })
                }
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
                $crate::network::initialize_network_id::<Self>(&INDEX)
            }
        }

        // A repeated layer maps to its first position, so this fails to compile. Lints are
        // not reported in macro expansions from another crate; a const assertion always is.
        const _: () = {
            #[allow(unreachable_patterns)]
            const fn position(layer: $name) -> usize {
                $crate::__network_layer_match!(layer, 0usize; []; $($layer),+)
            }
            let all: &[$name] = &[$($layer),+];
            let mut index = 0;
            while index < all.len() {
                assert!(
                    position(all[index]) == index,
                    "impl_network!: every layer must appear exactly once in `layers`"
                );
                index += 1;
            }
        };

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

/// The edge data check: the given `validate` closure, or the default probe (floats must
/// be finite and non-negative, anything else passes).
#[doc(hidden)]
#[macro_export]
macro_rules! __network_validate {
    ($data:ident, $validate:expr) => {
        ($validate)($data)
    };
    ($data:ident) => {{
        #[allow(unused_imports)]
        use $crate::network::{AnyEdgeData as _, FloatEdgeData as _};
        (&$crate::network::EdgeDataProbe::<Self::Data>(std::marker::PhantomData))
            .is_valid_edge_data($data)
    }};
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

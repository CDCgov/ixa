/*!

Macros for implementing properties.

# [`define_property!`][macro@crate::define_property]

For the most common cases, use the [`define_property!`][macro@crate::define_property] macro. This macro defines a struct or enum
with the standard derives required by the [`Property`][crate::entity::property::Property] trait and implements [`Property`][crate::entity::property::Property] (via
[`impl_property!`][macro@crate::impl_property]) for you.

```rust,ignore
define_property!(struct Age(u8), Person);
define_property!(struct Location(City, State), Person);
define_property!(
    enum InfectionStatus {
        Susceptible,
        Infectious,
        Recovered,
    },
    Person,
    default_const = InfectionStatus::Susceptible
);
```

Notice the convenient `default_const = <default_value>` keyword argument that allows you to
define a compile-time constant default value for the property. This is an optional argument.
If it is omitted, a value for the property must be supplied upon entity creation.

The primary advantage of using this macro is that it automatically derives the list of traits every
[`Property`][crate::entity::property::Property] needs to derive for you. You don't have to remember them. You also get a cute syntax for
specifying the default value, but it's not much harder to specify default values using other macros.

If you need the macro to generate `Eq` and/or `Hash` manually instead of deriving them, use the
optional `impl_eq_hash = ...` argument with one of the following values: `Eq`, `Hash`, `both`, or
`neither`.

Notice you need to use the `struct` or `enum` keywords, but you don't need to
specify the visibility. A `pub` visibility is added automatically to the struct
and to inner fields of tuple structs in the expansion.

# [`impl_property!`][macro@crate::impl_property]

You can implement [`Property`][crate::entity::property::Property] for existing types using the
[`impl_property!`][macro@crate::impl_property] macro. This macro defines the
[`Property`][crate::entity::property::Property] trait implementation for you but doesn't take care
of the `#[derive(..)]` boilerplate, so you have to remember to derive or implement the traits
required by [`AnyProperty`][crate::entity::property::AnyProperty] for your type: `Copy`, `Clone`,
`Debug`, `PartialEq`, `Eq`, and `Hash`.

If the type cannot derive `PartialEq` / `Eq` or `Hash`, for example because it contains `f32` or
`f64`, use [`impl_property_eq!`][macro@crate::impl_property_eq],
[`impl_property_hash!`][macro@crate::impl_property_hash], or
[`impl_property_eq_hash!`][macro@crate::impl_property_eq_hash] to generate those implementations
for the manually declared type. These macros require `ixa::rkyv::Archive` and `ixa::rkyv::Serialize` derives
because they compare and hash the type's archived byte representation.

Some examples:

```rust,ignore
define_entity!(Person);

// The `define_property!` automatically adds `pub` visibility to the struct and its tuple fields. If
// we want to restrict the visibility of our `Property` type, we can use the `impl_property!` macro
// instead. The only catch is, we have to remember to derive or implement the traits required by
// `AnyProperty`.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
struct Age(pub u8);
impl_property!(Age, Person);

// Here we derive `Default`, which also requires an attribute on one
// of the variants. (`Property` has its own independent mechanism for
// assigning default values for entities unrelated to the `Default` trait.)
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Default)]
enum InfectionStatus {
    #[default]
    Susceptible,
    Infected,
    Recovered,
}
// We also specify the default value explicitly for entities.
impl_property!(InfectionStatus, Person, default_const = InfectionStatus::Susceptible);

// Exactly equivalent to
//    `define_property!(struct Vaccinated(pub bool), Person, default_const = Vaccinated(false));`
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct Vaccinated(pub bool);
impl_property!(Vaccinated, Person, default_const = Vaccinated(false));

// For manually declared floating-point properties, generate byte-based equality and hashing.
#[derive(
    Copy,
    Clone,
    Debug,
    serde::Serialize,
    serde::Deserialize,
    ixa::rkyv::Archive,
    ixa::rkyv::Serialize,
)]
#[rkyv(crate = ixa::rkyv)]
struct Weight(pub f64);
impl_property_eq_hash!(Weight);
impl_property!(Weight, Person, default_const = Weight(0.0));
```

# [`impl_property!`][macro@crate::impl_property] with options

The [`impl_property!`][macro@crate::impl_property] macro gives you much more control over the implementation of your
property type. It takes optional keyword arguments for things like the default value,
initialization strategy, and how the property is converted to a string for display.

Non-derived properties either have a default constant value for new entities
(`default_const = ...`), or a value is required to be provided for new entities
(no `default_const`).

```rust,ignore
impl_property!(
    InfectionStatus,
    Person,
    default_const = InfectionStatus::Susceptible,
    display_impl = |v| format!("status: {v:?}")
);
```

*/

/// Defines a `struct` or `enum` with a standard set of derives and automatically invokes
/// [`impl_property!`][macro@crate::impl_property] for it. This macro provides a concise shorthand for defining
/// simple property types that follow the same derive and implementation pattern.
///
/// The macro supports the following forms:
///
/// ### 1. Tuple Structs
/// ```rust
/// # use ixa::{define_entity, define_property};
/// # define_entity!(Person);
/// define_property!(struct Age(u8), Person);
/// ```
/// Expands to:
/// ```rust
/// # use ixa::{impl_property, define_entity};
/// # define_entity!(Person);
/// #[derive(Debug, PartialEq, Eq, Hash, Clone, Copy, serde::Serialize, serde::Deserialize)]
/// pub struct Age(u8);
/// impl_property!(Age, Person);
/// ```
///
/// You can define multiple tuple fields:
/// ```rust,ignore
/// define_property!(struct Location(City, State), Person);
/// ```
///
/// ### 2. Named-field Structs
/// ```rust
/// # use ixa::{define_property, define_entity};
/// # define_entity!(Person);
/// define_property!(struct Coordinates { x: i32, y: i32 }, Person);
/// ```
/// Expands to:
/// ```rust
/// # use ixa::{impl_property, define_entity};
/// # define_entity!(Person);
/// #[derive(Debug, PartialEq, Eq, Hash, Clone, Copy, serde::Serialize, serde::Deserialize)]
/// pub struct Coordinates { x: i32, y: i32 }
/// impl_property!(Coordinates, Person);
/// ```
///
/// ### 3. Enums
/// ```rust
/// # use ixa::{define_property, define_entity};
/// # define_entity!(Person);
/// define_property!(
///     enum InfectionStatus {
///         Susceptible,
///         Infectious,
///         Recovered,
///     },
///     Person
/// );
/// ```
/// Expands to:
/// ```rust
/// # use ixa::{impl_property, define_entity};
/// # define_entity!(Person);
/// #[derive(Debug, PartialEq, Eq, Hash, Clone, Copy, serde::Serialize, serde::Deserialize)]
/// pub enum InfectionStatus {
///     Susceptible,
///     Infectious,
///     Recovered,
/// }
/// impl_property!(InfectionStatus, Person);
/// ```
///
/// ### Notes
///
/// - By default, the generated type derives `Debug`, `PartialEq`, `Eq`, `Hash`, `Clone`, and `Copy`.
/// - Use the optional `default_const = <default_value>` argument to define a compile-time constant
///   default for the property.
/// - Use `impl_eq_hash = Eq`, `Hash`, `both`, or `neither` as the first optional argument to suppress the default
///   `Eq`/`Hash` derives and switch to generated or user-supplied implementations.
/// - Remaining optional arguments follow the same ordering as [`impl_property!`][macro@crate::impl_property].
/// - If you need a more complex type definition (e.g., generics, attributes, or non-`Copy`
///   fields), define the type manually and then call [`impl_property!`][macro@crate::impl_property] directly.
#[macro_export]
macro_rules! define_property {
    // Implementation Notes
    //
    // To implement the optional `impl_eq_hash` keyword argument, we have the following choices:
    //
    // 1. Have a single public match branch per type form with `$(, impl_eq_hash =
    //    $impl_eq_hash:ident)?`, but explicitly list all the keyword options. This option disallows
    //    the `$(, $($extra:tt)+)*` pattern for the tail.
    // 2. Have two branches per type form, one with the `impl_eq_hash = ...` keyword present and one
    //    with it absent, and use the `$(, $($extra:tt)+)*` pattern for the tail. This duplicates the
    //    number of public match arms, but it allows us to keep the keyword arguments defined in
    //    `impl_property!` instead of repeated throughout the code.
    // 3. Use a proc macro or "TT munching", both of which are more heavy weight.
    //
    // We choose the second option. Unfortunately, this doesn't completely eliminate repetition of
    // the list of keyword arguments. We still have them in the
    // `impl_derived_property!(@with_option_display_default ...)` and
    // `impl_property!(@with_option_display_default ...)` subcommands.

    (
        struct $name:ident ( $visibility:vis Option<$inner_ty:ty> ),
        $entity:ident,
        impl_eq_hash = $impl_eq_hash:ident
        $(, $($extra:tt)*)?
    ) => {
        $crate::define_property!(
            @apply_property_decoration $impl_eq_hash,
            pub struct $name(pub Option<$inner_ty>);,
            $name
        );
        $crate::impl_property!(@with_option_display_default $name, $entity $(, $($extra)*)?);
    };
    (
        struct $name:ident ( $visibility:vis Option<$inner_ty:ty> ),
        $entity:ident
        $(, $($extra:tt)*)?
    ) => {
        $crate::define_property!(
            @apply_property_decoration ,
            pub struct $name(pub Option<$inner_ty>);,
            $name
        );
        $crate::impl_property!(@with_option_display_default $name, $entity $(, $($extra)*)?);
    };

    (
        struct $name:ident ( $($visibility:vis $field_ty:ty),* $(,)? ),
        $entity:ident,
        impl_eq_hash = $impl_eq_hash:ident
        $(, $($extra:tt)*)?
    ) => {
        $crate::define_property!(
            @apply_property_decoration $impl_eq_hash,
            pub struct $name($(pub $field_ty),*);,
            $name
        );
        $crate::impl_property!($name, $entity $(, $($extra)*)?);
    };
    (
        struct $name:ident ( $($visibility:vis $field_ty:ty),* $(,)? ),
        $entity:ident
        $(, $($extra:tt)*)?
    ) => {
        $crate::define_property!(
            @apply_property_decoration ,
            pub struct $name($(pub $field_ty),*);,
            $name
        );
        $crate::impl_property!($name, $entity $(, $($extra)*)?);
    };

    (
        struct $name:ident { $($visibility:vis $field_name:ident : $field_ty:ty),* $(,)? },
        $entity:ident,
        impl_eq_hash = $impl_eq_hash:ident
        $(, $($extra:tt)*)?
    ) => {
        $crate::define_property!(
            @apply_property_decoration $impl_eq_hash,
            pub struct $name { $(pub $field_name : $field_ty),* },
            $name
        );
        $crate::impl_property!($name, $entity $(, $($extra)*)?);
    };
    (
        struct $name:ident { $($visibility:vis $field_name:ident : $field_ty:ty),* $(,)? },
        $entity:ident
        $(, $($extra:tt)*)?
    ) => {
        $crate::define_property!(
            @apply_property_decoration ,
            pub struct $name { $(pub $field_name : $field_ty),* },
            $name
        );
        $crate::impl_property!($name, $entity $(, $($extra)*)?);
    };

    (
        enum $name:ident {
            $($variant:ident),* $(,)?
        },
        $entity:ident,
        impl_eq_hash = $impl_eq_hash:ident
        $(, $($extra:tt)*)?
    ) => {
        $crate::define_property!(
            @apply_property_decoration $impl_eq_hash,
            pub enum $name { $($variant),* },
            $name
        );
        $crate::impl_property!($name, $entity $(, $($extra)*)?);
    };
    (
        enum $name:ident {
            $($variant:ident),* $(,)?
        },
        $entity:ident
        $(, $($extra:tt)*)?
    ) => {
        $crate::define_property!(
            @apply_property_decoration ,
            pub enum $name { $($variant),* },
            $name
        );
        $crate::impl_property!($name, $entity $(, $($extra)*)?);
    };

    // Both `define_property!` and `define_derived_property!` need to attach derives to a
    // concrete item, so the mode table lives here as a shared internal subcommand.
    (@apply_property_decoration , $item:item, $name:ident) => {
        #[derive(Debug, PartialEq, Eq, Hash, Clone, Copy, serde::Serialize, serde::Deserialize)]
        $item
    };
    (@apply_property_decoration Eq, $item:item, $name:ident) => {
        #[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize, $crate::rkyv::Archive, $crate::rkyv::Serialize)]
        #[rkyv(crate = $crate::rkyv)]
        $item
        $crate::impl_property_eq!($name);
    };
    (@apply_property_decoration Hash, $item:item, $name:ident) => {
        #[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize, $crate::rkyv::Archive, $crate::rkyv::Serialize)]
        #[rkyv(crate = $crate::rkyv)]
        $item
        $crate::impl_property_hash!($name);
    };
    (@apply_property_decoration both, $item:item, $name:ident) => {
        #[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize, $crate::rkyv::Archive, $crate::rkyv::Serialize)]
        #[rkyv(crate = $crate::rkyv)]
        $item
        $crate::impl_property_eq_hash!($name);
    };
    (@apply_property_decoration neither, $item:item, $name:ident) => {
        #[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
        $item
    };
    (@apply_property_decoration $mode:ident, $item:item, $name:ident) => {
        compile_error!("`impl_eq_hash` must be one of `Eq`, `Hash`, `both`, or `neither`");
    };

}

/// Implements [`PartialEq`][core::cmp::PartialEq] and [`Eq`][core::cmp::Eq] for a property type
/// using Ixa's generated byte-based equality behavior.
///
/// This macro is useful when declaring a property type manually and then using
/// [`impl_property!`][macro@crate::impl_property] or
/// [`impl_derived_property!`][macro@crate::impl_derived_property]. The type must implement
/// [`rkyv::Archive`][crate::rkyv::Archive] and [`rkyv::Serialize`][crate::rkyv::Serialize], because
/// equality is computed by serializing the archived representation of each value.
///
/// The macro accepts a concrete property type identifier. Generic property types are not supported.
#[macro_export]
macro_rules! impl_property_eq {
    ($name:ident) => {
        impl core::cmp::PartialEq for $name {
            fn eq(&self, other: &Self) -> bool {
                const N: usize = core::mem::size_of::<<$name as $crate::rkyv::Archive>::Archived>();

                let left = $crate::rkyv::api::high::to_bytes_in::<_, $crate::rkyv::rancor::Error>(
                    self,
                    $crate::hashing::EqualityBufferWriter::<N>::new(),
                )
                .expect("serializing left value for equality comparison failed");

                let right = $crate::rkyv::api::high::to_bytes_in::<_, $crate::rkyv::rancor::Error>(
                    other,
                    $crate::hashing::EqualityBufferWriter::<N>::new(),
                )
                .expect("serializing right value for equality comparison failed");

                left.as_written() == right.as_written()
            }
        }

        impl core::cmp::Eq for $name {}
    };
}

/// Implements [`Hash`][core::hash::Hash] for a property type using Ixa's generated byte-based
/// hashing behavior.
///
/// This macro is useful when declaring a property type manually and then using
/// [`impl_property!`][macro@crate::impl_property] or
/// [`impl_derived_property!`][macro@crate::impl_derived_property]. The type must implement
/// [`rkyv::Archive`][crate::rkyv::Archive] and [`rkyv::Serialize`][crate::rkyv::Serialize], because
/// hashing is computed by serializing the archived representation into the supplied hasher.
///
/// The macro accepts a concrete property type identifier. Generic property types are not supported.
#[macro_export]
macro_rules! impl_property_hash {
    ($name:ident) => {
        impl core::hash::Hash for $name {
            fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
                $crate::rkyv::api::high::to_bytes_in::<_, $crate::rkyv::rancor::Error>(
                    self,
                    $crate::hashing::HasherWriter::new(state),
                )
                .expect("serialization failed while hashing");
            }
        }
    };
}

/// Implements [`PartialEq`][core::cmp::PartialEq], [`Eq`][core::cmp::Eq], and
/// [`Hash`][core::hash::Hash] for a property type using Ixa's generated byte-based equality and
/// hashing behavior.
///
/// This is a convenience wrapper around [`impl_property_eq!`][macro@crate::impl_property_eq] and
/// [`impl_property_hash!`][macro@crate::impl_property_hash].
///
/// The macro accepts a concrete property type identifier. Generic property types are not supported.
#[macro_export]
macro_rules! impl_property_eq_hash {
    ($name:ident) => {
        $crate::impl_property_eq!($name);
        $crate::impl_property_hash!($name);
    };
}

/// Implements the [`Property`][crate::entity::property::Property] trait for the given property type and entity.
///
/// Use this macro when you want to implement the `Property<E: Entity>` trait for a type you have declared yourself.
/// You might want to declare your own property type yourself instead of using the [`define_property!`][macro@crate::define_property] macro if
/// - you want a visibility other than `pub`
/// - you want to derive additional traits
/// - your type definition requires attribute proc-macros or other special syntax (for example, deriving
///   `Default` on an enum requires an attribute on one of the variants)
///
/// Example:
///
/// In this example, in addition to the set of derives required for all property types, we also derive the `Default`
/// trait for an enum type, which requires the proc-macro attribute `#[default]` on one of the variants.
///
/// ```rust
/// # use ixa::{impl_property, define_entity};
/// # define_entity!(Person);
/// #[derive(Default, Debug, PartialEq, Eq, Hash, Clone, Copy)]
/// pub enum InfectionStatus {
///     #[default]
///     Susceptible,
///     Infectious,
///     Recovered,
/// }
/// // We also specify that this property is assigned a default value for new entities if a value isn't provided.
/// // Here we have it coincide with `Default::default()`, but this isn't required.
/// impl_property!(InfectionStatus, Person, default_const = InfectionStatus::Susceptible);
/// ```
///
/// # Parameters
///
/// Parameters must be given in the correct order.
///
/// * `$property`: The identifier for the type implementing [`Property`][crate::entity::property::Property].
/// * `$entity`: The entity type this property is associated with.
/// * Optional parameters (each may be omitted; defaults will be used):
///   * `compute_derived_fn = <expr>` — Function used to compute derived properties. Use `define_derived_property!` or
///     `impl_derived_property!` instead of using this option directly.
///   * `default_const = <expr>` — Constant default value if the property has one; implies a non-derived property.
///   * `display_impl = <expr>` — Function converting the property value to a string; defaults to `|v| format!("{v:?}")`.
/// * Optional parameters that should generally be left alone, used internally to implement derived properties and
///   multi-properties:
///   * `collect_deps_fn = <expr>` — Function used to collect property dependencies; defaults to an empty implementation.
///   * `ctor_registration = <expr>` — Code run in the `ctor` for property registration.
///
/// # Semantics
/// - If `compute_derived_fn` is provided, the property is derived. In this case, `default_const` must be absent, and
///   calling `Property::default_const()` results in a panic. Use `define_derived_property!` or `impl_derived_property!`
///   instead of using this option directly.
/// - If `default_const` is provided, the property is a non-derived constant property. In this case,
///   `compute_derived_fn` must be absent, and calling `Property::compute_derived()` results in a panic.
/// - If neither is provided, the property is non-derived and required/explicit; both `Property::default_const()` and
///   `Property::compute_derived()` panic.
/// - If both are provided, a compile-time error is emitted.
#[macro_export]
macro_rules! impl_property {
    (
        $property:ident,
        $entity:ident
        $(, compute_derived_fn = $compute_derived_fn:expr)?
        $(, default_const = $default_const:expr)?
        $(, collect_deps_fn = $collect_deps_fn:expr)?
        $(, display_impl = $display_impl:expr)?
        $(, ctor_registration = $ctor_registration:expr)?
    ) => {
        // Enforce mutual exclusivity at compile time.
        $crate::impl_property!(@assert_not_both $($compute_derived_fn)? ; $($default_const)?);

        $crate::impl_property!(
            @__impl_property_common
            $property,
            $entity,

            // initialization_kind (implicit)
            $crate::impl_property!(@select_initialization_kind $($compute_derived_fn)? ; $($default_const)?),

            // compute_derived_fn (panic unless explicitly provided)
            $crate::impl_property!(
                @unwrap_or
                $($compute_derived_fn)?,
                |_, _| panic!("property {} is not derived", stringify!($property))
            ),

            // default_const (panic unless explicitly provided)
            $crate::impl_property!(
                @unwrap_or
                $($default_const)?,
                panic!("property {} has no default value", stringify!($property))
            ),

            // query_parts_type
            [&'a dyn std::any::Any; 1],

            // value_from_query_parts_fn
            {
                |parts: &[&dyn std::any::Any]| -> Option<$property> {
                    let [part] = parts else {
                        return None;
                    };
                    part.downcast_ref::<$property>().copied()
                }
            },

            // query_parts_for_value_fn
            {
                fn default_query_parts_for_value<'a>(value: &'a $property) -> [&'a dyn std::any::Any; 1] {
                    [value as &'a dyn std::any::Any]
                }

                default_query_parts_for_value
            },

            // type_id_fn
            {
                std::any::TypeId::of::<Self>()
            },

            // query_value_hash_fn
            {
                |value: &$property| $crate::hashing::one_shot_128(value)
            },

            // display_impl
            $crate::impl_property!(@unwrap_or $($display_impl)?, |v| format!("{v:?}")),

            // collect_deps_fn
            $crate::impl_property!(
                @unwrap_or
                $($collect_deps_fn)?,
                |_| {/* Do nothing */}
            ),

            // ctor_registration
            $crate::impl_property!(@unwrap_or $($ctor_registration)?, {
                $crate::entity::property_store::add_to_property_registry::<$entity, $property>();
            }),
        );
    };

    (
        @with_option_display_default
        $property:ident,
        $entity:ident
        $(, compute_derived_fn = $compute_derived_fn:expr)?
        $(, default_const = $default_const:expr)?
        $(, collect_deps_fn = $collect_deps_fn:expr)?
        $(, display_impl = $display_impl:expr)?
        $(, ctor_registration = $ctor_registration:expr)?
    ) => {
        $crate::impl_property!(
            $property,
            $entity
            $(, compute_derived_fn = $compute_derived_fn)?
            $(, default_const = $default_const)?
            $(, collect_deps_fn = $collect_deps_fn)?
            , display_impl = $crate::impl_property!(@unwrap_or $($display_impl)?, |value: &Self| {
                match value.0 {
                    Some(v) => format!("{:?}", v),
                    None => "None".to_string(),
                }
            })
            $(, ctor_registration = $ctor_registration)?
        );
    };

    (
        @multi_property
        $property:ident,
        $entity:ident,
        ( $($dependency:ident),+ )
        $(, compute_derived_fn = $compute_derived_fn:expr)?
        $(, default_const = $default_const:expr)?
        $(, collect_deps_fn = $collect_deps_fn:expr)?
        $(, display_impl = $display_impl:expr)?
        $(, ctor_registration = $ctor_registration:expr)?
    ) => {
        $crate::impl_property!(@assert_not_both $($compute_derived_fn)? ; $($default_const)?);

        $crate::impl_property!(
            @__impl_property_common
            $property,
            $entity,
            $crate::impl_property!(@select_initialization_kind $($compute_derived_fn)? ; $($default_const)?),
            $crate::impl_property!(
                @unwrap_or
                $($compute_derived_fn)?,
                |_, _| panic!("property {} is not derived", stringify!($property))
            ),
            $crate::impl_property!(
                @unwrap_or
                $($default_const)?,
                panic!("property {} has no default value", stringify!($property))
            ),
            [&'a dyn std::any::Any; $crate::impl_property!(@count_tts $($dependency),+)],
            {
                $crate::paste::paste! {
                    #[allow(unused_assignments)]
                    |parts: &[&dyn std::any::Any]| -> Option<$property> {
                        let [$( [<p_ $dependency:lower>] ),+,] = parts else {
                            return None;
                        };
                        let sorted_parts = [$( [<p_ $dependency:lower>] ),+];
                        let keys = [
                            $(
                                <$dependency as $crate::entity::property::Property<$entity>>::type_id(),
                            )+
                        ];
                        let indices = $crate::entity::multi_property::static_sorted_indices(&keys);
                        let inverse = $crate::entity::multi_property::static_inverse_indices(&indices);
                        let mut declared_index = 0usize;
                        Some((
                            $(
                                {
                                    let value = *sorted_parts[inverse[declared_index]].downcast_ref::<$dependency>()?;
                                    declared_index += 1;
                                    value
                                },
                            )+
                        ))
                    }
                }
            },
            {
                $crate::paste::paste! {
                    fn multi_property_query_parts_for_value<'a>(
                        value: &'a $property,
                    ) -> [&'a dyn std::any::Any; $crate::impl_property!(@count_tts $($dependency),+)] {
                        let keys = [
                            $(
                                <$dependency as $crate::entity::property::Property<$entity>>::type_id(),
                            )+
                        ];
                        let ( $( [<_ $dependency:lower>] ),+ ) = value;
                        let mut parts = [
                            $(
                                [<_ $dependency:lower>] as &'a dyn std::any::Any,
                            )+
                        ];
                        $crate::entity::multi_property::static_reorder_by_keys(&keys, &mut parts);
                        parts
                    }

                    multi_property_query_parts_for_value
                }
            },
            {
                std::any::TypeId::of::<Self>()
            },
            {
                $crate::paste::paste! {
                    |value: &$property| -> $crate::hashing::HashValueType {
                        let keys = [
                            $(
                                <$dependency as $crate::entity::property::Property<$entity>>::type_id(),
                            )+
                        ];
                        let ( $( [<_ $dependency:lower>] ),+ ) = value;
                        let mut value_hashes = [
                            $(
                                $crate::hashing::one_shot_128([<_ $dependency:lower>]),
                            )+
                        ];
                        $crate::entity::multi_property::static_reorder_by_keys(&keys, &mut value_hashes);
                        $crate::hashing::one_shot_128(&value_hashes)
                    }
                }
            },
            $crate::impl_property!(@unwrap_or $($display_impl)?, |v| format!("{v:?}")),
            $crate::impl_property!(@unwrap_or $($collect_deps_fn)?, |_| {/* Do nothing */}),
            $crate::impl_property!(@unwrap_or $($ctor_registration)?, {
                $crate::entity::property_store::add_to_property_registry::<$entity, $property>();
            }),
        );
    };

    // Compile-time mutual exclusivity check.
    (@assert_not_both $compute_derived_fn:expr ; $default_const:expr) => {
        compile_error!(
            "impl_property!: `compute_derived_fn = ...` (derived property) and `default_const = ...` \
             (non-derived property default constant) are mutually exclusive. Remove one of them."
        );
    };
    (@assert_not_both $compute_derived_fn:expr ; ) => {};
    (@assert_not_both ; $default_const:expr) => {};
    (@assert_not_both ; ) => {};

    // Select initialization kind (implicit).
    (@select_initialization_kind $compute_derived_fn:expr ; $default_const:expr) => {
        // This arm should be unreachable because @assert_not_both triggers first, but keep it
        // as a backstop if the macro is used incorrectly.
        compile_error!(
            "impl_property!: cannot select initialization kind because both `compute_derived_fn` \
             and `default_const` are present"
        )
    };
    (@select_initialization_kind $compute_derived_fn:expr ; ) => {
        $crate::entity::property::PropertyInitializationKind::Derived
    };
    (@select_initialization_kind ; $default_const:expr) => {
        $crate::entity::property::PropertyInitializationKind::Constant
    };
    (@select_initialization_kind ; ) => {
        $crate::entity::property::PropertyInitializationKind::Explicit
    };

    // Helpers for defaults, a pair per macro parameter type (`expr`, `ty`).
    (@unwrap_or $value:expr, $_default:expr) => { $value };
    (@unwrap_or, $default:expr) => { $default };

    (@replace_with_unit $_tt:tt) => { () };
    (@count_tts $($tt:tt),* $(,)?) => {
        <[()]>::len(&[$($crate::impl_property!(@replace_with_unit $tt)),*])
    };

    // This is the purely syntactic implementation.
    (
        @__impl_property_common
        $property:ident,           // The name of the type we are implementing `Property` for
        $entity:ident,             // The entity type this property is associated with
        $initialization_kind:expr, // The kind of initialization this property has (implicit selection)
        $compute_derived_fn:expr,  // If the property is derived, the function that computes the value
        $default_const:expr,       // If the property has a constant default initial value, the default value
        $query_parts_type:ty,
        $value_from_query_parts_fn:expr,
        $query_parts_for_value_fn:expr,
        $type_id_fn:expr,          // Code that returns the logical type ID for this property
        $query_value_hash_fn:expr,
        $display_impl:expr,        // A function that takes a value and returns a string representation of this property
        $collect_deps_fn:expr,     // If the property is derived, the function that computes the value
        $ctor_registration:expr,   // Code that runs in a ctor for property registration
    ) => {
        impl $crate::entity::property::Property<$entity> for $property {
            type QueryParts<'a> = $query_parts_type where Self: 'a;

            const NAME: &'static str = stringify!($property);

            fn initialization_kind() -> $crate::entity::property::PropertyInitializationKind {
                $initialization_kind
            }

            fn compute_derived(
                _context: &$crate::Context,
                _entity_id: $crate::entity::EntityId<$entity>,
            ) -> Self {
                ($compute_derived_fn)(_context, _entity_id)
            }

            fn default_const() -> Self {
                $default_const
            }

            fn value_from_query_parts(
                parts: &[&dyn std::any::Any],
            ) -> Option<Self> {
                ($value_from_query_parts_fn)(parts)
            }

            fn query_parts_for_value(value: &Self) -> Self::QueryParts<'_> {
                ($query_parts_for_value_fn)(value)
            }

            fn type_id() -> std::any::TypeId {
                $type_id_fn
            }

            fn query_value_hash(value: &Self) -> $crate::hashing::HashValueType {
                ($query_value_hash_fn)(value)
            }

            fn get_display(&self) -> String {
                ($display_impl)(self)
            }

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
                $crate::entity::property_store::initialize_property_id::<$entity>(&INDEX)
            }

            fn collect_non_derived_dependencies(result: &mut $crate::HashSet<usize>) {
                ($collect_deps_fn)(result)
            }
        }

        $crate::paste::paste! {
            $crate::ctor::declarative::ctor!{
                #[ctor(unsafe)]
                fn [<_register_property_ $entity:snake _ $property:snake>]() {
                    $ctor_registration
                }
            }
        }
    };
}

/// The "derived" variant of [`define_property!`][macro@crate::define_property] for defining simple derived property types.
/// Defines a `struct` or `enum` with a standard set of derives and automatically invokes
/// [`impl_derived_property!`][macro@crate::impl_derived_property] for it.
///
/// Defines a derived property with the following parameters:
/// * Property type declaration: A struct or enum declaration.
/// * `$entity`: The name of the entity of which the new type is a property.
/// * `[$($dependency),+]`: A list of person properties the derived property depends on.
/// * `[$(global_dependency),*]`: A list of global properties the derived property depends on. Can optionally be omitted if empty.
/// * `$calculate`: A closure that takes the values of each dependency and returns the derived value.
/// * Optional parameters: The same optional parameters accepted by [`impl_property!`][macro@crate::impl_property],
///   plus `impl_eq_hash = Eq | Hash | both | neither` to control whether `Eq`/`Hash` are derived or generated
///   for the declared type, mirroring [`define_property!`][macro@crate::define_property].
#[macro_export]
macro_rules! define_derived_property {
    // Implementation Notes
    //
    // We reuse `define_property!`'s shared decoration helper, then delegate the derived-property
    // behavior to `impl_derived_property!`.
    //
    // See `derive_property!` implementation notes for why each type form is duplicated.

    // Struct (tuple) with single Option<T> field
    (
        struct $name:ident ( $visibility:vis Option<$inner_ty:ty> ),
        $entity:ident,
        [$($dependency:ident),*]
        $(, [$($global_dependency:ident),*])?,
        |$($param:ident),+| $derive_fn:expr,
        impl_eq_hash = $impl_eq_hash:ident
        $(, $($extra:tt)+)*
    ) => {
        $crate::define_property!(
            @apply_property_decoration
            $impl_eq_hash,
            pub struct $name(pub Option<$inner_ty>);,
            $name
        );

        $crate::impl_derived_property!(
            @with_option_display_default
            $name,
            $entity,
            [$($dependency),*],
            [$($($global_dependency),*)?],
            |$($param),+| $derive_fn
            $(, $($extra)+)*
        );
    };
    (
        struct $name:ident ( $visibility:vis Option<$inner_ty:ty> ),
        $entity:ident,
        [$($dependency:ident),*]
        $(, [$($global_dependency:ident),*])?,
        |$($param:ident),+| $derive_fn:expr
        $(, $($extra:tt)+)*
    ) => {
        $crate::define_property!(
            @apply_property_decoration
            ,
            pub struct $name(pub Option<$inner_ty>);,
            $name
        );

        $crate::impl_derived_property!(
            @with_option_display_default
            $name,
            $entity,
            [$($dependency),*],
            [$($($global_dependency),*)?],
            |$($param),+| $derive_fn
            $(, $($extra)+)*
        );
    };

    // Struct (tuple)
    (
        struct $name:ident ( $($visibility:vis $field_ty:ty),* $(,)? ),
        $entity:ident,
        [$($dependency:ident),*]
        $(, [$($global_dependency:ident),*])?,
        |$($param:ident),+| $derive_fn:expr,
        impl_eq_hash = $impl_eq_hash:ident
        $(, $($extra:tt)+)*
    ) => {
        $crate::define_property!(
            @apply_property_decoration
            $impl_eq_hash,
            pub struct $name( $(pub $field_ty),* );,
            $name
        );

        $crate::impl_derived_property!(
            $name,
            $entity,
            [$($dependency),*],
            [$($($global_dependency),*)?],
            |$($param),+| $derive_fn
            $(, $($extra)+)*
        );
    };
    (
        struct $name:ident ( $($visibility:vis $field_ty:ty),* $(,)? ),
        $entity:ident,
        [$($dependency:ident),*]
        $(, [$($global_dependency:ident),*])?,
        |$($param:ident),+| $derive_fn:expr
        $(, $($extra:tt)+)*
    ) => {
        $crate::define_property!(
            @apply_property_decoration
            ,
            pub struct $name( $(pub $field_ty),* );,
            $name
        );

        $crate::impl_derived_property!(
            $name,
            $entity,
            [$($dependency),*],
            [$($($global_dependency),*)?],
            |$($param),+| $derive_fn
            $(, $($extra)+)*
        );
    };

    // Struct (named fields)
    (
        struct $name:ident { $($visibility:vis $field_name:ident : $field_ty:ty),* $(,)? },
        $entity:ident,
        [$($dependency:ident),*]
        $(, [$($global_dependency:ident),*])?,
        |$($param:ident),+| $derive_fn:expr,
        impl_eq_hash = $impl_eq_hash:ident
        $(, $($extra:tt)+)*
    ) => {
        $crate::define_property!(
            @apply_property_decoration
            $impl_eq_hash,
            pub struct $name { $($visibility $field_name : $field_ty),* },
            $name
        );

        $crate::impl_derived_property!(
            $name,
            $entity,
            [$($dependency),*],
            [$($($global_dependency),*)?],
            |$($param),+| $derive_fn
            $(, $($extra)+)*
        );
    };
    (
        struct $name:ident { $($visibility:vis $field_name:ident : $field_ty:ty),* $(,)? },
        $entity:ident,
        [$($dependency:ident),*]
        $(, [$($global_dependency:ident),*])?,
        |$($param:ident),+| $derive_fn:expr
        $(, $($extra:tt)+)*
    ) => {
        $crate::define_property!(
            @apply_property_decoration
            ,
            pub struct $name { $($visibility $field_name : $field_ty),* },
            $name
        );

        $crate::impl_derived_property!(
            $name,
            $entity,
            [$($dependency),*],
            [$($($global_dependency),*)?],
            |$($param),+| $derive_fn
            $(, $($extra)+)*
        );
    };

    // Enum
    (
        enum $name:ident {
            $($variant:ident),* $(,)?
        },
        $entity:ident,
        [$($dependency:ident),*]
        $(, [$($global_dependency:ident),*])?,
        |$($param:ident),+| $derive_fn:expr,
        impl_eq_hash = $impl_eq_hash:ident
        $(, $($extra:tt)+)*
    ) => {
        $crate::define_property!(
            @apply_property_decoration
            $impl_eq_hash,
            pub enum $name {
                $($variant),*
            },
            $name
        );

        $crate::impl_derived_property!(
            $name,
            $entity,
            [$($dependency),*],
            [$($($global_dependency),*)?],
            |$($param),+| $derive_fn
            $(, $($extra)+)*
        );
    };
    (
        enum $name:ident {
            $($variant:ident),* $(,)?
        },
        $entity:ident,
        [$($dependency:ident),*]
        $(, [$($global_dependency:ident),*])?,
        |$($param:ident),+| $derive_fn:expr
        $(, $($extra:tt)+)*
    ) => {
        $crate::define_property!(
            @apply_property_decoration
            ,
            pub enum $name {
                $($variant),*
            },
            $name
        );

        $crate::impl_derived_property!(
            $name,
            $entity,
            [$($dependency),*],
            [$($($global_dependency),*)?],
            |$($param),+| $derive_fn
            $(, $($extra)+)*
        );
    };
}

/// Implements the [`Property`][crate::entity::property::Property] trait for an existing type as a derived property.
///
/// Accepts the same parameters as [`define_derived_property!`][macro@crate::define_derived_property], except the first parameter is the name of a
/// type assumed to already be declared rather than a type declaration. This is the derived property equivalent
/// of [`impl_property!`][macro@crate::impl_property]. It calls [`impl_property!`][macro@crate::impl_property] with the appropriate derived property parameters.
#[macro_export]
macro_rules! impl_derived_property {
    (
        $name:ident,
        $entity:ident,
        [$($dependency:ident),*]
        $(, [$($global_dependency:ident),*])?,
        |$($param:ident),+| $derive_fn:expr
        $(, $($extra:tt)+)*
    ) => {
        $crate::impl_property!(
            $name,
            $entity,
            compute_derived_fn = $crate::impl_derived_property!(
                @construct_compute_fn
                $entity,
                [$($dependency),*],
                [$($($global_dependency),*)?],
                |$($param),+| $derive_fn
            ),
            collect_deps_fn = | deps: &mut $crate::HashSet<usize> | {
                $(
                    if <$dependency as $crate::entity::property::Property<$entity>>::is_derived() {
                        <$dependency as $crate::entity::property::Property<$entity>>::collect_non_derived_dependencies(deps);
                    } else {
                        deps.insert(<$dependency as $crate::entity::property::Property<$entity>>::id());
                    }
                )*
            }
            $(, $($extra)+)*
        );
    };

    // Internal branch to construct the compute function.
    (
        @construct_compute_fn
        $entity:ident,
        [$($dependency:ident),*],
        [$($global_dependency:ident),*],
        |$($param:ident),+| $derive_fn:expr
    ) => {
        |context: &$crate::Context, entity_id| {
            #[allow(unused_imports)]
            use $crate::global_properties::ContextGlobalPropertiesExt;
            #[allow(unused_parens)]
            let ($($param,)*) = (
                $(context.get_property::<$entity, $dependency>(entity_id)),*,
                $(
                    context.get_global_property_value($global_dependency)
                        .expect(&format!("Global property {} not initialized", stringify!($global_dependency)))
                ),*
            );
            $derive_fn
        }
    };

    (@unwrap_or $value:expr, $_default:expr) => { $value };
    (@unwrap_or, $default:expr) => { $default };

    (
        @with_option_display_default
        $name:ident,
        $entity:ident,
        [$($dependency:ident),*],
        [$($global_dependency:ident),*],
        |$($param:ident),+| $derive_fn:expr
        $(, default_const = $default_const:expr)?
        $(, display_impl = $display_impl:expr)?
        $(, collect_deps_fn = $collect_deps_fn:expr)?
        $(, ctor_registration = $ctor_registration:expr)?
    ) => {
        $crate::impl_derived_property!(
            $name,
            $entity,
            [$($dependency),*],
            [$($global_dependency),*],
            |$($param),+| $derive_fn
            $(, default_const = $default_const)?
            , display_impl = $crate::impl_derived_property!(@unwrap_or $($display_impl)?, |value: &$name| {
                match value.0 {
                    Some(v) => format!("{:?}", v),
                    None => "None".to_string(),
                }
            })
            $(, collect_deps_fn = $collect_deps_fn)?
            $(, ctor_registration = $ctor_registration)?
        );
    };

}

/// Defines a derived property consisting of a (named) tuple of other properties. The primary use case
/// is for indexing and querying properties jointly.
///
/// The querying subsystem is able to detect when its multiple component properties are
/// equivalent to an indexed multi-property and use that index to perform the query.
///
#[macro_export]
macro_rules! define_multi_property {
        (
            ( $($dependency:ident),+ ),
            $entity:ident
        ) => {
            $crate::paste::paste! {
                type [<$($dependency)*>] = ( $($dependency),+ );

                $crate::impl_property!(
                    @multi_property
                    [<$($dependency)*>],
                    $entity,
                    ( $($dependency),+ ),
                    compute_derived_fn = |context: &$crate::Context, entity_id: $crate::entity::EntityId<$entity>| {
                        (
                            $(context.get_property::<$entity, $dependency>(entity_id)),+
                        )
                    },

                    collect_deps_fn = | deps: &mut $crate::HashSet<usize> | {
                        $(
                            if <$dependency as $crate::entity::property::Property<$entity>>::is_derived() {
                                <$dependency as $crate::entity::property::Property<$entity>>::collect_non_derived_dependencies(deps);
                            } else {
                                deps.insert(<$dependency as $crate::entity::property::Property<$entity>>::id());
                            }
                        )*
                    },

                    display_impl = |val: &( $($dependency),+ )| {
                        let ( $( [<_ $dependency:lower>] ),+ ) = val;
                        let mut displayed = String::from("(");
                        $(
                            displayed.push_str(
                                &<$dependency as $crate::entity::property::Property<$entity>>::get_display([<_ $dependency:lower>])
                            );
                            displayed.push_str(", ");
                        )+
                        displayed.truncate(displayed.len() - 2);
                        displayed.push_str(")");
                        displayed
                    },

                    ctor_registration = {
                        let mut type_ids = [$( <$dependency as $crate::entity::property::Property<$entity>>::type_id() ),+];
                        type_ids.sort_unstable();
                        if let Some((_, existing_name)) =
                            $crate::entity::multi_property::register_type_ids_to_multi_property_id(
                                <$entity as $crate::entity::Entity>::id(),
                                &type_ids,
                                <[<$($dependency)*>] as $crate::entity::property::Property<$entity>>::type_id(),
                                <[<$($dependency)*>] as $crate::entity::property::Property<$entity>>::id(),
                                <[<$($dependency)*>] as $crate::entity::property::Property<$entity>>::name(),
                            )
                        {
                            $crate::entity::multi_property::record_pre_main_warning(format!(
                                "multi-property {} is equivalent to already registered multi-property {existing_name}; queries will resolve to {existing_name}, and attempting to index {} will panic",
                                <[<$($dependency)*>] as $crate::entity::property::Property<$entity>>::name(),
                                <[<$($dependency)*>] as $crate::entity::property::Property<$entity>>::name(),
                            ));
                        }
                        $crate::entity::property_store::add_to_property_registry::<$entity, [<$($dependency)*>]>();
                    }
                );

            }
        };
    }

#[cfg(test)]
mod tests {
    // We define unused properties to test macro implementation.
    #![allow(dead_code)]

    use crate::entity::QueryInternal;
    use crate::prelude::*;
    use crate::with;

    define_entity!(Person);
    define_entity!(Group);

    define_property!(struct Pu32(u32), Person, default_const = Pu32(0));
    define_property!(struct POu32(Option<u32>), Person, default_const = POu32(None));
    define_property!(
        struct POFloat(Option<f64>),
        Person,
        impl_eq_hash = both,
        default_const = POFloat(None)
    );
    define_property!(
        struct POu32Custom(Option<u32>),
        Person,
        default_const = POu32Custom(None),
        display_impl = |value: &POu32Custom| match value.0 {
            Some(v) => format!("custom:{v}"),
            None => "custom:none".to_string(),
        }
    );
    define_property!(struct Name(&'static str), Person, default_const = Name(""));
    define_property!(struct Age(u8), Person, default_const = Age(0));
    define_property!(struct Weight(f64), Person, impl_eq_hash = both, default_const = Weight(0.0));

    // A struct with named fields
    define_property!(
        struct Innocculation {
            time: f64,
            dose: u8,
        },
        Person,
        impl_eq_hash = both,
        default_const = Innocculation { time: 0.0, dose: 0 }
    );

    // An enum non-derived property
    define_property!(
        enum InfectionStatus {
            Susceptible,
            Infected,
            Recovered,
        },
        Person,
        default_const = InfectionStatus::Susceptible
    );

    // An enum derived property
    define_derived_property!(
        enum AgeGroup {
            Child,
            Adult,
            Senior,
        },
        Person,
        [Age], // Depends only on age
        |age| {
            let age: Age = age;
            if age.0 < 18 {
                AgeGroup::Child
            } else if age.0 < 65 {
                AgeGroup::Adult
            } else {
                AgeGroup::Senior
            }
        }
    );

    // Derived property - computed from other properties
    define_derived_property!(struct DerivedProp(bool), Person, [Age],
        |age| {
            DerivedProp(age.0 % 2 == 0)
        }
    );

    define_derived_property!(
        struct DerivedMaybeAge(Option<u8>),
        Person,
        [Age],
        |age| DerivedMaybeAge((age.0 != 0).then_some(age.0))
    );

    define_derived_property!(
        struct DerivedMaybeWeight(Option<f64>),
        Person,
        [Age],
        |age| DerivedMaybeWeight((age.0 != 0).then_some(age.0 as f64)),
        impl_eq_hash = both
    );

    define_derived_property!(
        struct DerivedMaybeAgeCustom(Option<u8>),
        Person,
        [Age],
        |age| DerivedMaybeAgeCustom((age.0 != 0).then_some(age.0)),
        display_impl = |value: &DerivedMaybeAgeCustom| match value.0 {
            Some(v) => format!("derived:{v}"),
            None => "derived:none".to_string(),
        }
    );

    define_derived_property!(
        struct DerivedWeight(f64),
        Person,
        [Age],
        |age| DerivedWeight(age.0 as f64),
        impl_eq_hash = both
    );

    // A property type for two distinct entities.
    #[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
    pub enum InfectionKind {
        Respiratory,
        Genetic,
        Superficial,
    }
    impl_property!(
        InfectionKind,
        Person,
        default_const = InfectionKind::Respiratory
    );
    impl_property!(InfectionKind, Group, default_const = InfectionKind::Genetic);

    define_multi_property!((Name, Age, Weight), Person);
    define_multi_property!((Age, Weight, Name), Person);
    define_multi_property!((Weight, Age, Name), Person);

    // For convenience
    type ProfileNAW = (Name, Age, Weight);
    type ProfileAWN = (Age, Weight, Name);
    type ProfileWAN = (Weight, Age, Name);
    type Years = Age;
    define_multi_property!((Years, Weight), Person);

    define_entity!(SingleProfilePerson);
    define_property!(
        struct SingleName(&'static str),
        SingleProfilePerson,
        default_const = SingleName("")
    );
    define_property!(
        struct SingleAge(u8),
        SingleProfilePerson,
        default_const = SingleAge(0)
    );
    define_property!(
        struct SingleWeight(u8),
        SingleProfilePerson,
        default_const = SingleWeight(0)
    );
    define_multi_property!((SingleName, SingleAge, SingleWeight), SingleProfilePerson);
    type SingleProfile = (SingleName, SingleAge, SingleWeight);

    #[test]
    fn test_multi_property_ordering() {
        let a = (Name("Jane"), Age(22), Weight(180.5));
        let b = (Age(22), Weight(180.5), Name("Jane"));
        let c = (Weight(180.5), Age(22), Name("Jane"));

        // Equivalent multi-properties keep distinct storage and type identities.
        // Query routing equivalence is handled by the multi-property registry.
        assert_ne!(ProfileNAW::id(), ProfileAWN::id());
        assert_ne!(ProfileNAW::id(), ProfileWAN::id());
        assert_ne!(ProfileNAW::type_id(), ProfileAWN::type_id());
        assert_ne!(ProfileNAW::type_id(), ProfileWAN::type_id());

        assert_eq!(
            ProfileNAW::query_value_hash(&a),
            ProfileAWN::query_value_hash(&b)
        );
        assert_eq!(
            ProfileNAW::query_value_hash(&a),
            ProfileWAN::query_value_hash(&c)
        );
    }

    #[test]
    fn test_single_multi_property_vs_property_query() {
        let mut context = Context::new();

        context
            .add_entity(with!(
                SingleProfilePerson,
                SingleName("John"),
                SingleAge(42),
                SingleWeight(220)
            ))
            .unwrap();
        context
            .add_entity(with!(
                SingleProfilePerson,
                SingleName("Jane"),
                SingleAge(22),
                SingleWeight(180)
            ))
            .unwrap();
        context
            .add_entity(with!(
                SingleProfilePerson,
                SingleName("Bob"),
                SingleAge(32),
                SingleWeight(190)
            ))
            .unwrap();
        context
            .add_entity(with!(
                SingleProfilePerson,
                SingleName("Alice"),
                SingleAge(22),
                SingleWeight(170)
            ))
            .unwrap();

        context.index_property::<SingleProfilePerson, SingleProfile>();

        let example_query = (SingleName("Alice"), SingleAge(22), SingleWeight(170));
        assert_eq!(
            <SingleProfile as QueryInternal<SingleProfilePerson>>::multi_property_id(
                &example_query
            ),
            Some(SingleProfile::id())
        );
        let query_parts = QueryInternal::query_parts(&example_query);
        assert_eq!(
            SingleProfile::value_from_query_parts(query_parts.as_ref()),
            Some((SingleName("Alice"), SingleAge(22), SingleWeight(170)))
        );

        context.with_query_results(
            with!(
                SingleProfilePerson,
                (SingleName("John"), SingleAge(42), SingleWeight(220))
            ),
            &mut |results| {
                assert_eq!(results.into_iter().count(), 1);
            },
        );
    }

    #[test]
    fn test_equivalent_multi_property_query_routing() {
        let example_query = (Name("Alice"), Age(22), Weight(170.5));
        let query_multi_property_id =
            <(Name, Age, Weight) as QueryInternal<Person>>::multi_property_id(&example_query);

        assert!(matches!(
            query_multi_property_id,
            Some(id) if id == ProfileNAW::id() || id == ProfileAWN::id() || id == ProfileWAN::id()
        ));

        let query_parts = QueryInternal::query_parts(&example_query);
        assert_eq!(
            ProfileNAW::value_from_query_parts(query_parts.as_ref()),
            Some((Name("Alice"), Age(22), Weight(170.5)))
        );
        assert_eq!(
            ProfileAWN::value_from_query_parts(query_parts.as_ref()),
            Some((Age(22), Weight(170.5), Name("Alice")))
        );
        assert_eq!(
            ProfileWAN::value_from_query_parts(query_parts.as_ref()),
            Some((Weight(170.5), Age(22), Name("Alice")))
        );
    }

    #[test]
    fn test_multi_property_component_type_alias_query_routing() {
        let mut context = Context::new();

        context
            .add_entity(with!(Person, Age(44), Weight(155.0)))
            .unwrap();
        context.index_property::<Person, YearsWeight>();

        assert_eq!(
            context.query_entity_count(with!(Person, Weight(155.0), Age(44))),
            1
        );
    }

    #[test]
    fn test_derived_property() {
        let mut context = Context::new();

        let senior = context
            .add_entity::<Person, _>(with!(Person, Age(92)))
            .unwrap();
        let child = context
            .add_entity::<Person, _>(with!(Person, Age(12)))
            .unwrap();
        let adult = context
            .add_entity::<Person, _>(with!(Person, Age(44)))
            .unwrap();

        let senior_group: AgeGroup = context.get_property(senior);
        let child_group: AgeGroup = context.get_property(child);
        let adult_group: AgeGroup = context.get_property(adult);

        assert_eq!(senior_group, AgeGroup::Senior);
        assert_eq!(child_group, AgeGroup::Child);
        assert_eq!(adult_group, AgeGroup::Adult);

        // Age has no dependencies (only dependents)
        assert!(Age::non_derived_dependencies().is_empty());
        // AgeGroup depends only on Age
        assert_eq!(AgeGroup::non_derived_dependencies(), [Age::id()]);

        // Age has several dependents. This assert may break if you add or remove the properties in this test module.
        let mut expected_dependents = [
            AgeGroup::id(),
            DerivedProp::id(),
            DerivedMaybeAge::id(),
            DerivedMaybeWeight::id(),
            DerivedMaybeAgeCustom::id(),
            DerivedWeight::id(),
            ProfileNAW::id(),
            ProfileAWN::id(),
            ProfileWAN::id(),
            YearsWeight::id(),
        ];
        expected_dependents.sort_unstable();
        assert_eq!(Age::dependents(), expected_dependents);
    }

    #[test]
    fn test_get_display() {
        let mut context = Context::new();
        let person = context
            .add_entity(with!(Person, POu32(Some(42)), Pu32(22)))
            .unwrap();
        assert_eq!(
            POu32::get_display(&context.get_property::<_, POu32>(person)).to_string(),
            "42"
        );
        assert_eq!(
            Pu32::get_display(&context.get_property::<_, Pu32>(person)).to_string(),
            "Pu32(22)"
        );
        let person2 = context
            .add_entity(with!(Person, POu32(None), Pu32(11)))
            .unwrap();
        assert_eq!(
            POu32::get_display(&context.get_property::<_, POu32>(person2)).to_string(),
            "None"
        );
    }

    #[test]
    fn test_option_property_display_patterns() {
        let mut context = Context::new();

        let some_person = context
            .add_entity(with!(
                Person,
                POu32(Some(42)),
                POFloat(Some(3.5)),
                POu32Custom(Some(7)),
                Pu32(1),
            ))
            .unwrap();
        let none_person = context
            .add_entity(with!(
                Person,
                POu32(None),
                POFloat(None),
                POu32Custom(None),
                Pu32(2)
            ))
            .unwrap();

        assert_eq!(
            POu32::get_display(&context.get_property::<_, POu32>(some_person)),
            "42"
        );
        assert_eq!(
            POu32::get_display(&context.get_property::<_, POu32>(none_person)),
            "None"
        );

        assert_eq!(
            POFloat::get_display(&context.get_property::<_, POFloat>(some_person)),
            "3.5"
        );
        assert_eq!(
            POFloat::get_display(&context.get_property::<_, POFloat>(none_person)),
            "None"
        );

        assert_eq!(
            POu32Custom::get_display(&context.get_property::<_, POu32Custom>(some_person)),
            "custom:7"
        );
        assert_eq!(
            POu32Custom::get_display(&context.get_property::<_, POu32Custom>(none_person)),
            "custom:none"
        );
    }

    #[test]
    fn test_option_derived_property_display_patterns() {
        let mut context = Context::new();

        let some_person = context
            .add_entity::<Person, _>(with!(Person, Age(42)))
            .unwrap();
        let none_person = context
            .add_entity::<Person, _>(with!(Person, Age(0)))
            .unwrap();

        assert_eq!(
            DerivedMaybeAge::get_display(&context.get_property::<_, DerivedMaybeAge>(some_person)),
            "42"
        );
        assert_eq!(
            DerivedMaybeAge::get_display(&context.get_property::<_, DerivedMaybeAge>(none_person)),
            "None"
        );

        assert_eq!(
            DerivedMaybeWeight::get_display(
                &context.get_property::<_, DerivedMaybeWeight>(some_person)
            ),
            "42.0"
        );
        assert_eq!(
            DerivedMaybeWeight::get_display(
                &context.get_property::<_, DerivedMaybeWeight>(none_person)
            ),
            "None"
        );

        assert_eq!(
            DerivedMaybeAgeCustom::get_display(
                &context.get_property::<_, DerivedMaybeAgeCustom>(some_person)
            ),
            "derived:42"
        );
        assert_eq!(
            DerivedMaybeAgeCustom::get_display(
                &context.get_property::<_, DerivedMaybeAgeCustom>(none_person)
            ),
            "derived:none"
        );
    }

    #[test]
    fn test_debug_trait() {
        let property = Pu32(11);
        let debug_str = format!("{:?}", property);
        assert_eq!(debug_str, "Pu32(11)");

        let property = POu32(Some(22));
        let debug_str = format!("{:?}", property);
        assert_eq!(debug_str, "POu32(Some(22))");
    }

    #[test]
    fn test_define_derived_property_impl_eq_hash() {
        let mut values = crate::HashSet::default();
        values.insert(DerivedWeight(3.0));
        assert!(values.contains(&DerivedWeight(3.0)));
    }
}

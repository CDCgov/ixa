/*!

Macros for implementing properties.

# [`define_property!`]

For the most common cases, use the [`define_property!`] macro. This macro defines a struct or enum
with the standard derives required by the [`Property`][crate::entity::property::Property] trait and implements [`Property`][crate::entity::property::Property] (via
[`impl_property!`]) for you.

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

Notice you need to use the `struct` or `enum` keywords, but you don't need to
specify the visibility. A `pub` visibility is added automatically to the struct
and to inner fields of tuple structs in the expansion.

# [`impl_property!`]

You can implement [`Property`][crate::entity::property::Property] for existing types using the [`impl_property!`] macro. This macro defines the
[`Property`][crate::entity::property::Property] trait implementation for you but doesn't take care of the `#[derive(..)]` boilerplate, so you
have to remember to `derive` all of `Copy, Clone, Debug, PartialEq, Serialize` in your type declaration.

Some examples:

```rust,ignore
define_entity!(Person);

// The `define_property!` automatically adds `pub` visibility to the struct and its tuple
// fields. If we want to restrict the visibility of our `Property` type, we can use the
// `impl_property!` macro instead. The only
// catch is, we have to remember to `derive` all of `Copy, Clone, Debug, PartialEq, Serialize`.
#[derive(Copy, Clone, Debug, PartialEq, Serialize)]
struct Age(pub u8);
impl_property!(Age, Person);

// Here we derive `Default`, which also requires an attribute on one
// of the variants. (`Property` has its own independent mechanism for
// assigning default values for entities unrelated to the `Default` trait.)
#[derive(Copy, Clone, Debug, PartialEq, Default, Serialize)]
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
#[derive(Copy, Clone, Debug, PartialEq, Serialize)]
pub struct Vaccinated(pub bool);
impl_property!(Vaccinated, Person, default_const = Vaccinated(false));
```

# [`impl_property!`] with options

The [`impl_property!`] macro gives you much more control over the implementation of your
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

## Use case: `Property::CanonicalValue` different from `Self`

The `Property::CanonicalValue` type is used to store the property value in
the index. If the property type is different from the value type, you can
specify a custom canonical type using the `canonical_value` parameter, but
you also must provide a conversion function to and from the canonical type.

```rust,ignore
define_entity!(WeatherStation);

#[derive(Debug, PartialEq, Eq, Clone, Copy, Serialize)]
pub struct DegreesFahrenheit(pub f64);

#[derive(Debug, PartialEq, Eq, Clone, Copy, Serialize)]
pub struct DegreesCelsius(pub f64);

// Custom canonical type
impl_property!(
    DegreesFahrenheit,
    WeatherStation,
    canonical_value = DegreesCelsius,
    make_canonical = |s: &DegreesFahrenheit| DegreesCelsius((s.0 - 32.0) * 5.0 / 9.0),
    make_uncanonical = |v: DegreesCelsius| DegreesFahrenheit(v.0 * 9.0 / 5.0 + 32.0),
    display_impl = |v| format!("{:.1} °C", v.0)
);
```

*/

/// Defines a `struct` or `enum` with a standard set of derives and automatically invokes
/// [`impl_property!`] for it. This macro provides a concise shorthand for defining
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
/// # use ixa::{impl_property, define_entity}; use serde::Serialize;
/// # define_entity!(Person);
/// #[derive(Debug, PartialEq, Eq, Clone, Copy, Serialize)]
/// pub struct Age(pub u8);
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
/// # use ixa::{impl_property, define_entity}; use serde::Serialize;
/// # define_entity!(Person);
/// #[derive(Debug, PartialEq, Eq, Clone, Copy, Serialize)]
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
/// # use ixa::{impl_property, define_entity}; use serde::Serialize;
/// # define_entity!(Person);
/// #[derive(Debug, PartialEq, Eq, Clone, Copy, Serialize)]
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
/// - The generated type always derives the following traits:
///   `Debug`, `PartialEq`, `Eq`, `Clone`, `Copy`, and `Serialize`.
/// - Use the optional `default_const = <default_value>` argument to define a compile-time constant
///   default for the property.
/// - If you need a more complex type definition (e.g., generics, attributes, or non-`Copy`
///   fields), define the type manually and then call [`impl_property!`] directly.
#[macro_export]
macro_rules! define_property {
    // Struct (tuple) with single Option<T> field (special case)
    (
        struct $name:ident ( $visibility:vis Option<$inner_ty:ty> ),
        $entity:ident
        $(, $($extra:tt)+),*
    ) => {
        #[derive(Debug, PartialEq, Clone, Copy, serde::Serialize)]
        pub struct $name(pub Option<$inner_ty>);

        // Use impl_property! to provide a custom display implementation
        $crate::impl_property!(
            $name,
            $entity
            $(, $($extra)+)*
            , display_impl = |value: &Self| {
                match value.0 {
                    Some(v) => format!("{:?}", v),
                    None => "None".to_string(),
                }
            }
        );
    };

    // Struct (tuple)
    (
        struct $name:ident ( $($visibility:vis $field_ty:ty),* $(,)? ),
        $entity:ident
        $(, $($extra:tt)+),*
    ) => {
        #[derive(Debug, PartialEq, Clone, Copy, serde::Serialize)]
        pub struct $name($(pub $field_ty),*);
        $crate::impl_property!($name, $entity $(, $($extra)+)*);
    };

    // Struct (named fields)
    (
        struct $name:ident { $($visibility:vis $field_name:ident : $field_ty:ty),* $(,)? },
        $entity:ident
        $(, $($extra:tt)+),*
    ) => {
        #[derive(Debug, PartialEq, Clone, Copy, serde::Serialize)]
        pub struct $name { $($visibility $field_name : $field_ty),* }
        $crate::impl_property!($name, $entity $(, $($extra)+)*);
    };

    // Enum
    (
        enum $name:ident {
            $($variant:ident),* $(,)?
        },
        $entity:ident
        $(, $($extra:tt)+),*
    ) => {
        #[derive(Debug, PartialEq, Clone, Copy, serde::Serialize)]
        pub enum $name {
            $($variant),*
        }
        $crate::impl_property!($name, $entity $(, $($extra)+)*);
    };
}

/// Implements the [`Property`][crate::entity::property::Property] trait for the given property type and entity.
///
/// Use this macro when you want to implement the `Property<E: Entity>` trait for a type you have declared yourself.
/// You might want to declare your own property type yourself instead of using the [`define_property!`] macro if
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
/// # use ixa::{impl_property, define_entity}; use serde::Serialize;
/// # define_entity!(Person);
/// #[derive(Default, Debug, PartialEq, Eq, Clone, Copy, Serialize)]
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
///   * `canonical_value = <type>` — If the type stored in the index differs from the property's value type; defaults to
///     `Self`. If this option is supplied, you will also want to supply `make_canonical` and `make_uncanonical`.
///   * `make_canonical = <expr>` — Function converting from `Self` to `CanonicalValue`; defaults to `std::convert::identity`.
///   * `make_uncanonical = <expr>` — Function converting from `CanonicalValue` to `Self`; defaults to `std::convert::identity`.
/// * Optional parameters that should generally be left alone, used internally to implement derived properties and
///   multi-properties:
///   * `index_id_fn = <expr>` — Function used to initialize the property index id; defaults to `Self::id()`.
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
        $(, display_impl = $display_impl:expr)?
        $(, canonical_value = $canonical_value:ty)?
        $(, make_canonical = $make_canonical:expr)?
        $(, make_uncanonical = $make_uncanonical:expr)?
        $(, index_id_fn = $index_id_fn:expr)?
        $(, collect_deps_fn = $collect_deps_fn:expr)?
        $(, ctor_registration = $ctor_registration:expr)?
    ) => {
        // Enforce mutual exclusivity at compile time.
        $crate::impl_property!(@assert_not_both $($compute_derived_fn)? ; $($default_const)?);

        $crate::impl_property!(
            @__impl_property_common
            $property,
            $entity,

            // canonical value
            $crate::impl_property!(@unwrap_or_ty $($canonical_value)?, $property),

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

            // make_canonical
            $crate::impl_property!(@unwrap_or $($make_canonical)?, std::convert::identity),

            // make_uncanonical
            $crate::impl_property!(@unwrap_or $($make_uncanonical)?, std::convert::identity),

            // display_impl
            $crate::impl_property!(@unwrap_or $($display_impl)?, |v| format!("{v:?}")),

            // index_id_fn
            $crate::impl_property!(@unwrap_or $($index_id_fn)?, {
                <Self as $crate::entity::property::Property<$entity>>::id()
            }),

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

    (@unwrap_or_ty $ty:ty, $_default:ty) => { $ty };
    (@unwrap_or_ty, $default:ty) => { $default };

    // This is the purely syntactic implementation.
    (
        @__impl_property_common
        $property:ident,           // The name of the type we are implementing `Property` for
        $entity:ident,             // The entity type this property is associated with
        $canonical_value:ty,       // If the type stored in the index is different from Self, the name of that type
        $initialization_kind:expr, // The kind of initialization this property has (implicit selection)
        $compute_derived_fn:expr,  // If the property is derived, the function that computes the value
        $default_const:expr,       // If the property has a constant default initial value, the default value
        $make_canonical:expr,      // A function that takes a value and returns a canonical value
        $make_uncanonical:expr,    // A function that takes a canonical value and returns a value
        $display_impl:expr,        // A function that takes a canonical value and returns a string representation of this property
        $index_id_fn:expr,         // Code that returns the unique index for this property
        $collect_deps_fn:expr,     // If the property is derived, the function that computes the value
        $ctor_registration:expr,   // Code that runs in a ctor for property registration
    ) => {
        impl $crate::entity::property::Property<$entity> for $property {
            type CanonicalValue = $canonical_value;

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

            fn make_canonical(self) -> Self::CanonicalValue {
                ($make_canonical)(self)
            }

            fn make_uncanonical(value: Self::CanonicalValue) -> Self {
                ($make_uncanonical)(value)
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

            fn index_id() -> usize {
                $index_id_fn
            }

            fn collect_non_derived_dependencies(result: &mut $crate::HashSet<usize>) {
                ($collect_deps_fn)(result)
            }
        }

        $crate::paste::paste! {
            $crate::ctor::declarative::ctor!{
                #[ctor]
                fn [<_register_property_ $entity:snake _ $property:snake>]() {
                    $ctor_registration
                }
            }
        }
    };
}

/// The "derived" variant of [`define_property!`] for defining simple derived property types.
/// Defines a `struct` or `enum` with a standard set of derives and automatically invokes
/// [`impl_derived_property!`] for it.
///
/// Defines a derived property with the following parameters:
/// * Property type declaration: A struct or enum declaration.
/// * `$entity`: The name of the entity of which the new type is a property.
/// * `[$($dependency),+]`: A list of person properties the derived property depends on.
/// * `[$(global_dependency),*]`: A list of global properties the derived property depends on. Can optionally be omitted if empty.
/// * `$calculate`: A closure that takes the values of each dependency and returns the derived value.
/// * Optional parameters: The same optional parameters accepted by [`impl_property!`].
#[macro_export]
macro_rules! define_derived_property {
    // The calls to `$crate::impl_derived_property!` are all the same except for
    // this first case of a newtype for an `Option<T>`, which has a special `display_impl`.

    // Struct (tuple) with single Option<T> field (special case)
    (
        struct $name:ident ( $visibility:vis Option<$inner_ty:ty> ),
        $entity:ident,
        [$($dependency:ident),*]
        $(, [$($global_dependency:ident),*])?,
        |$($param:ident),+| $derive_fn:expr
        // For `canonical_value` implementations:
        $(, $($extra:tt)+),*
    ) => {
        #[derive(Debug, PartialEq, Eq, Clone, Copy, serde::Serialize)]
        pub struct $name(pub Option<$inner_ty>);

        // Use impl_derived_property! to provide a custom display implementation
        $crate::impl_derived_property!(
            $name,
            $entity,
            [$($dependency),*],
            [$($($global_dependency),*)?],
            |$($param),+| $derive_fn,
            display_impl = |value: &Option<$inner_ty>| {
                match value {
                    Some(v) => format!("{:?}", v),
                    None => "None".to_string(),
                }
            }
            $(, $($extra)+)*
        );
    };

    // Struct (tuple)
    (
        struct $name:ident ( $($visibility:vis $field_ty:ty),* $(,)? ),
        $entity:ident,
        [$($dependency:ident),*]
        $(, [$($global_dependency:ident),*])?,
        |$($param:ident),+| $derive_fn:expr
        // For `canonical_value` implementations:
        $(, $($extra:tt)+),*
    ) => {
        #[derive(Debug, PartialEq, Eq, Clone, Copy, serde::Serialize)]
        pub struct $name( $(pub $field_ty),* );

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
        |$($param:ident),+| $derive_fn:expr
        // For `canonical_value` implementations:
        $(, $($extra:tt)+),*
    ) => {
        #[derive(Debug, PartialEq, Eq, Clone, Copy, serde::Serialize)]
        pub struct $name { $($visibility $field_name : $field_ty),* }

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
        |$($param:ident),+| $derive_fn:expr
        // For `canonical_value` implementations:
        $(, $($extra:tt)+),*
    ) => {
        #[derive(Debug, PartialEq, Eq, Clone, Copy, serde::Serialize)]
        pub enum $name {
            $($variant),*
        }

        $crate::impl_derived_property!(
            $name,
            $entity,
            [$($dependency),*],
            [$($($global_dependency),*)?],
            |$($param),+| $derive_fn
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
}

/// Implements the [`Property`][crate::entity::property::Property] trait for an existing type as a derived property.
///
/// Accepts the same parameters as [`define_derived_property!`], except the first parameter is the name of a
/// type assumed to already be declared rather than a type declaration. This is the derived property equivalent
/// of [`impl_property!`]. It calls [`impl_property!`] with the appropriate derived property parameters.
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
                        if $dependency::is_derived() {
                            $dependency::collect_non_derived_dependencies(deps);
                        } else {
                            deps.insert($dependency::id());
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
    }

/// Defines a derived property consisting of a (named) tuple of other properties. The primary use case
/// is for indexing and querying properties jointly.
///
/// The index subsystem is smart enough to reuse indexes for multi-properties that are equivalent up to
/// reordering of the component properties. The querying subsystem is able to detect when its multiple
/// component properties are equivalent to an indexed multi-property and use that index to perform the
/// query.
#[macro_export]
macro_rules! define_multi_property {
        (
            ( $($dependency:ident),+ ),
            $entity:ident
        ) => {
            $crate::paste::paste! {
                type [<$($dependency)*>] = ( $($dependency),+ );

                $crate::impl_property!(
                    [<$($dependency)*>],
                    $entity,
                    compute_derived_fn = |context: &$crate::Context, entity_id: $crate::entity::EntityId<$entity>| {
                        (
                            $(context.get_property::<$entity, $dependency>(entity_id)),+
                        )
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
                    canonical_value = $crate::sorted_tag!(( $($dependency),+ )),
                    make_canonical = $crate::reorder_closure!(( $($dependency),+ )),
                    make_uncanonical = $crate::unreorder_closure!(( $($dependency),+ )),

                    index_id_fn = {
                        // This static must be initialized with a compile-time constant expression.
                        // We use `usize::MAX` as a sentinel to mean "uninitialized". This
                        // static variable is shared among all instances of this concrete item type.
                        static INDEX_ID: std::sync::atomic::AtomicUsize =
                            std::sync::atomic::AtomicUsize::new(usize::MAX);

                        // Fast path: already initialized.
                        let index_id = INDEX_ID.load(std::sync::atomic::Ordering::Relaxed);
                        if index_id != usize::MAX {
                            return index_id;
                        }

                        // Slow path: initialize it.
                        // Multi-properties report a single index ID for all equivalent multi-properties,
                        // because they share a single `Index<E, P>` instance.
                        let mut type_ids = [$( <$dependency as $crate::entity::property::Property<$entity>>::type_id() ),+];
                        type_ids.sort_unstable();
                        // Check if an index has already been assigned to this property set.
                        match $crate::entity::multi_property::type_ids_to_multi_property_index(&type_ids) {
                            Some(index) => {
                                // An index exists. Reuse it for our own index.
                                INDEX_ID.store(index, std::sync::atomic::Ordering::Relaxed);
                                index
                            },
                            None => {
                                // An index ID is not yet assigned. We will use our own index for this property.
                                let index = <Self as $crate::entity::property::Property<$entity>>::id();
                                INDEX_ID.store(index, std::sync::atomic::Ordering::Relaxed);
                                // And register the new index with this property set.
                                $crate::entity::multi_property::register_type_ids_to_multi_property_index(
                                    &type_ids,
                                    index
                                );
                                index
                            }
                        }
                    },

                    collect_deps_fn = | deps: &mut $crate::HashSet<usize> | {
                        $(
                            if $dependency::is_derived() {
                                $dependency::collect_non_derived_dependencies(deps);
                            } else {
                                deps.insert($dependency::id());
                            }
                        )*
                    },

                    ctor_registration = {
                        // Ensure `Self::index_id()` is initialized at startup.
                        let _ = [<$($dependency)*>]::index_id();
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

    use serde::Serialize;

    use crate::entity::Query;
    use crate::prelude::*;

    define_entity!(Person);
    define_entity!(Group);

    define_property!(struct Pu32(u32), Person, default_const = Pu32(0));
    define_property!(struct POu32(Option<u32>), Person, default_const = POu32(None));
    define_property!(struct Name(&'static str), Person, default_const = Name(""));
    define_property!(struct Age(u8), Person, default_const = Age(0));
    define_property!(struct Weight(f64), Person, default_const = Weight(0.0));

    // A struct with named fields
    define_property!(
        struct Innocculation {
            time: f64,
            dose: u8,
        },
        Person,
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

    // A property type for two distinct entities.
    #[derive(Debug, PartialEq, Clone, Copy, Serialize)]
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

    #[test]
    fn test_multi_property_ordering() {
        let a = (Name("Jane"), Age(22), Weight(180.5));
        let b = (Age(22), Weight(180.5), Name("Jane"));
        let c = (Weight(180.5), Age(22), Name("Jane"));

        // Multi-properties share the same index
        assert_eq!(ProfileNAW::index_id(), ProfileAWN::index_id());
        assert_eq!(ProfileNAW::index_id(), ProfileWAN::index_id());

        let a_canonical: <ProfileNAW as Property<_>>::CanonicalValue =
            ProfileNAW::make_canonical(a);
        let b_canonical: <ProfileAWN as Property<_>>::CanonicalValue =
            ProfileAWN::make_canonical(b);
        let c_canonical: <ProfileWAN as Property<_>>::CanonicalValue =
            ProfileWAN::make_canonical(c);

        assert_eq!(a_canonical, b_canonical);
        assert_eq!(a_canonical, c_canonical);

        // Actually, all of the `Profile***::hash_property_value` methods should be the same,
        // so we could use any single one.
        assert_eq!(
            ProfileNAW::hash_property_value(&a_canonical),
            ProfileAWN::hash_property_value(&b_canonical)
        );
        assert_eq!(
            ProfileNAW::hash_property_value(&a_canonical),
            ProfileWAN::hash_property_value(&c_canonical)
        );

        // Since the canonical values are the same, we could have used any single one, but this
        // demonstrates that we can convert from one order to another.
        assert_eq!(ProfileNAW::make_uncanonical(b_canonical), a);
        assert_eq!(ProfileAWN::make_uncanonical(c_canonical), b);
        assert_eq!(ProfileWAN::make_uncanonical(a_canonical), c);
    }

    #[test]
    fn test_multi_property_vs_property_query() {
        let mut context = Context::new();

        context
            .add_entity((Name("John"), Age(42), Weight(220.5)))
            .unwrap();
        context
            .add_entity((Name("Jane"), Age(22), Weight(180.5)))
            .unwrap();
        context
            .add_entity((Name("Bob"), Age(32), Weight(190.5)))
            .unwrap();
        context
            .add_entity((Name("Alice"), Age(22), Weight(170.5)))
            .unwrap();

        context.index_property::<_, ProfileNAW>();

        // Check that all equivalent multi-properties are indexed...
        assert!(context.is_property_indexed::<Person, ProfileNAW>());
        assert!(context.is_property_indexed::<Person, ProfileAWN>());
        assert!(context.is_property_indexed::<Person, ProfileWAN>());
        // ...but only one `Index<E, P>` instance was created.
        let mut indexed_count = 0;
        if context
            .get_property_value_store::<Person, ProfileNAW>()
            .index
            .is_some()
        {
            indexed_count += 1;
        }
        if context
            .get_property_value_store::<Person, ProfileAWN>()
            .index
            .is_some()
        {
            indexed_count += 1;
        }
        if context
            .get_property_value_store::<Person, ProfileWAN>()
            .index
            .is_some()
        {
            indexed_count += 1;
        }
        assert_eq!(indexed_count, 1);

        {
            let example_query = (Name("Alice"), Age(22), Weight(170.5));
            let query_multi_property_id =
                <(Name, Age, Weight) as Query<Person>>::multi_property_id(&example_query);
            assert!(query_multi_property_id.is_some());
            assert_eq!(ProfileNAW::index_id(), query_multi_property_id.unwrap());
            assert_eq!(
                Query::multi_property_value_hash(&example_query),
                ProfileNAW::hash_property_value(
                    &(Name("Alice"), Age(22), Weight(170.5)).make_canonical()
                )
            );
        }

        context.with_query_results(((Name("John"), Age(42), Weight(220.5)),), &mut |results| {
            assert_eq!(results.len(), 1);
        });
    }

    #[test]
    fn test_derived_property() {
        let mut context = Context::new();

        let senior = context.add_entity::<Person, _>((Age(92),)).unwrap();
        let child = context.add_entity::<Person, _>((Age(12),)).unwrap();
        let adult = context.add_entity::<Person, _>((Age(44),)).unwrap();

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
            ProfileNAW::id(),
            ProfileAWN::id(),
            ProfileWAN::id(),
        ];
        expected_dependents.sort_unstable();
        assert_eq!(Age::dependents(), expected_dependents);
    }

    #[test]
    fn test_get_display() {
        let mut context = Context::new();
        let person = context.add_entity((POu32(Some(42)), Pu32(22))).unwrap();
        assert_eq!(
            format!(
                "{:}",
                POu32::get_display(&context.get_property::<_, POu32>(person))
            ),
            "42"
        );
        assert_eq!(
            format!(
                "{:}",
                Pu32::get_display(&context.get_property::<_, Pu32>(person))
            ),
            "Pu32(22)"
        );
        let person2 = context.add_entity((POu32(None), Pu32(11))).unwrap();
        assert_eq!(
            format!(
                "{:}",
                POu32::get_display(&context.get_property::<_, POu32>(person2))
            ),
            "None"
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
}

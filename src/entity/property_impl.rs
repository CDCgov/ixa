/*!

Macros for implementing properties.

# [`define_property!`]

For the most common cases, use the `define_property!` macro. This macro defines a struct or enum
with the standard derives required by the `Property` trait and implements `Property` (via
`impl_property!`) for you.

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

The primary advantage of using this macro is that it automatically derives the list of traits every
`Property` needs to derive for you. You don't have to remember them. You also get a cute syntax for
specifying the default value, but it's not much harder to specify default values using other macros.

Notice you need to use the `struct` or `enum` keywords, but you don't need to
specify the visibility. A `pub` visibility is added automatically in the expansion.

# [`impl_property!`]

You might want to implement your own property type yourself if
- you want a visibility other than `pub`
- you want to derive additional traits
- your type definition requires attribute proc-macros or other special syntax (for example, deriving
  `Default` on an enum requires an attribute on one of the variants)

You can implement `Property` for existing types using the `impl_property!` macro. This macro
defines the `Property` trait implementation for you but doesn't take care of the `#[derive(..)]`
boilerplate, so you have to remember to `derive` all of `Copy, Clone, Debug, PartialEq, Serialize`.

```rust,ignore
define_entity!(Person);

// The `define_property!` automatically adds `pub` visibility. If we want to restrict the
// visibility of our `Property` type, we can use the `impl_property!` macro instead. The only
// catch is, we have to remember to `derive` all of `Copy, Clone, Debug, PartialEq, Serialize`.
// (Note that we don't have a default value in this case.)
#[derive(Copy, Clone, Debug, PartialEq, Serialize)]
struct Age(u8);
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
//    `define_property!(struct Vaccinated(bool) = Vaccinated(false), Person);`
#[derive(Copy, Clone, Debug, PartialEq, Serialize)]
pub struct Vaccinated(bool);
impl_property!(Vaccinated, Person, default_const = Vaccinated(false));
```

# [`impl_property_with_options`]

The `impl_property_with_options` macro gives you much more control over the
implementation of your property type. It takes optional keyword arguments
for things like the default value, initialization strategy, and whether the
property is required, and how the property is converted to a string for display.

```rust,ignore
impl_property_with_options!(
    InfectionStatus,
    Person,
    default_const = InfectionStatus::Susceptible,
    display_impl = |v| format!("status: {v:?}")
);
impl_property_with_options!(
    ImmunityLevel,
    Person,
    initialization_kind = PropertyInitializationKind::Derived,
    compute_derived_fn = |entity, _| entity.get_property::<ExposureScore>().map(|e| e / 2)
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
impl_property_with_options!(
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
/// # use ixa::{impl_property, define_entity, serde::Serialize};
/// # define_entity!(Person);
/// #[derive(Debug, PartialEq, Eq, Clone, Copy, Serialize)]
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
/// # use ixa::{impl_property, define_entity, serde::Serialize};
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
/// # use ixa::{impl_property, define_entity, serde::Serialize};
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
///   `Default`, `Debug`, `PartialEq`, `Eq`, `Clone`, `Copy`, and `Serialize`.
/// - Use the optional `default_const = <default_value>` argument to define a compile-time constant
///   default for the property.
/// - Trailing commas in field or variant lists are allowed.
/// - If you need a more complex type definition (e.g., generics, attributes, or
///   non-`Copy` fields), define the type manually and then call
///   [`impl_property!`] or
///   [`impl_property_with_options!`]
///   directly.
#[macro_export]
macro_rules! define_property {
    // Struct (tuple) with single Option<T> field (special case)
    (
        struct $name:ident ( Option<$inner_ty:ty> ),
        $entity:ident
        $(, $($extra:tt)+),*
    ) => {
        #[derive(Debug, PartialEq, Clone, Copy, $crate::serde::Serialize)]
        pub struct $name(Option<$inner_ty>);

        // Use impl_property_with_options! to provide a custom display implementation
        $crate::impl_property_with_options!(
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
        struct $name:ident ( $($field_ty:ty),* $(,)? ),
        $entity:ident
        $(, $($extra:tt)+),*
    ) => {
        #[derive(Debug, PartialEq, Clone, Copy, $crate::serde::Serialize)]
        pub struct $name($($field_ty),*);
        $crate::impl_property!($name, $entity $(, $($extra)+)*);
    };

    // Struct (named fields)
    (
        struct $name:ident { $($field_name:ident : $field_ty:ty),* $(,)? },
        $entity:ident
        $(, $($extra:tt)+),*
    ) => {
        #[derive(Debug, PartialEq, Clone, Copy, $crate::serde::Serialize)]
        pub struct $name { pub $($field_name : $field_ty),* }
        $crate::impl_property!($name, $entity $(, $($extra)+)*);
    };

    // Enum without default
    (
        enum $name:ident {
            $($variant:ident),* $(,)?
        },
        $entity:ident
        $(, $($extra:tt)+),*
    ) => {
        #[derive(Debug, PartialEq, Clone, Copy, $crate::serde::Serialize)]
        pub enum $name {
            $($variant),*
        }
        $crate::impl_property!($name, $entity $(, $($extra)+)*);
    };
}
pub use define_property;

/// Defines a property with the following parameters:
/// * `$property`: A name for the identifier type of the property
/// * `$entity`: The entity type this property is associated with
/// * `default_const`: (Optional) A constant initial value. If it is not defined, calling `get_property`
///   on the property without explicitly setting a value first will panic.
#[macro_export]
macro_rules! impl_property {
    // T without constant default value
    ($property:ident, $entity:ident $(, $($extra:tt)+),*) => {
        $crate::impl_property_with_options!($property, $entity $(, $($extra)+)*);
    };
}
pub use impl_property;

/// Defines a property type with optional named configuration parameters. The named parameters
/// need to be supplied in the order listed below even if some of them are not used.
///
/// # Parameters
/// - `$property`: The identifier for the type implementing [`Property`].
/// - `$entity`: The entity type this property is associated with.
/// - Optional parameters (each may be omitted; defaults will be used):
///   - `initialization_kind = <expr>` — Initialization strategy; defaults to `PropertyInitializationKind::Explicit`.
///   - `is_required = <bool>` — Whether new entities must explicitly set this property; defaults to `false`.
///   - `compute_derived_fn = <expr>` — Function used to compute derived properties; defaults to `None`.
///   - `default_const = <expr>` — Constant default value if the property has one; defaults to `None`.
///   - `display_impl = <expr>` — Function converting the canonical value to a string; defaults to `|v| format!("{v:?}")`.
///   - `canonical_value = <type>` — If the type stored in the index differs from the property's value type.
///   - `make_canonical = <expr>` — Function converting from `Self` to `CanonicalValue`; defaults to `|s: &Self| *s`.
///   - `make_uncanonical = <expr>` — Function converting from `CanonicalValue` to `Self`; defaults to `|v| v`.
///   - `index_id_fn = <expr>` — Function used to initialize the property index id; defaults to `Self::id()`.
///   - `ctor_registration = <expr>` — Code run in the `ctor`.
#[macro_export]
macro_rules! impl_property_with_options {
    // Case 1: default_const is supplied => always OK
    (
        $property:ident,
        $entity:ident
        $(, initialization_kind = $initialization_kind:expr)?
        $(, is_required = $is_required:expr)?
        $(, compute_derived_fn = $compute_derived_fn:expr)?
        $(, collect_deps_fn = $collect_deps_fn:expr)?
        , default_const = $default_const:expr
        $(, display_impl = $display_impl:expr)?
        $(, canonical_value = $canonical_value:ty)?
        $(, make_canonical = $make_canonical:expr)?
        $(, make_uncanonical = $make_uncanonical:expr)?
        $(, index_id_fn = $index_id_fn:expr)?
        $(, ctor_registration = $ctor_registration:expr)?

    ) => {
        $crate::impl_property_with_options!(@impl
            $property,
            $entity
            $(, initialization_kind = $initialization_kind)?
            $(, is_required = $is_required)?
            $(, compute_derived_fn = $compute_derived_fn)?
            $(, collect_deps_fn = $collect_deps_fn)?
            , default_const = $default_const
            $(, display_impl = $display_impl)?
            $(, canonical_value = $canonical_value)?
            $(, make_canonical = $make_canonical)?
            $(, make_uncanonical = $make_uncanonical)?
            $(, index_id_fn = $index_id_fn)?
            $(, ctor_registration = $ctor_registration)?
        );
    };

    // Case 2: compute_derived_fn is supplied (derived property) => OK
    (
        $property:ident,
        $entity:ident
        $(, initialization_kind = $initialization_kind:expr)?
        $(, is_required = $is_required:expr)?
        , compute_derived_fn = $compute_derived_fn:expr
        $(, collect_deps_fn = $collect_deps_fn:expr)?
        $(, default_const = $default_const:expr)? // allowed, but not required here
        $(, display_impl = $display_impl:expr)?
        $(, canonical_value = $canonical_value:ty)?
        $(, make_canonical = $make_canonical:expr)?
        $(, make_uncanonical = $make_uncanonical:expr)?
        $(, index_id_fn = $index_id_fn:expr)?
        $(, ctor_registration = $ctor_registration:expr)?
    ) => {
        $crate::impl_property_with_options!(@impl
            $property,
            $entity
            $(, initialization_kind = $initialization_kind)?
            $(, is_required = $is_required)?
            , compute_derived_fn = $compute_derived_fn
            $(, collect_deps_fn = $collect_deps_fn)?
            $(, default_const = $default_const)?
            $(, display_impl = $display_impl)?
            $(, canonical_value = $canonical_value)?
            $(, make_canonical = $make_canonical)?
            $(, make_uncanonical = $make_uncanonical)?
            $(, index_id_fn = $index_id_fn)?
            $(, ctor_registration = $ctor_registration)?
        );
    };

    // Case 3: no default_const and no compute_derived_fn, but is_required is explicitly true => OK
    (
        $property:ident,
        $entity:ident
        $(, initialization_kind = $initialization_kind:expr)?
        , is_required = true
        $(, collect_deps_fn = $collect_deps_fn:expr)?
        $(, display_impl = $display_impl:expr)?
        $(, canonical_value = $canonical_value:ty)?
        $(, make_canonical = $make_canonical:expr)?
        $(, make_uncanonical = $make_uncanonical:expr)?
        $(, index_id_fn = $index_id_fn:expr)?
        $(, ctor_registration = $ctor_registration:expr)?
    ) => {
        $crate::impl_property_with_options!(@impl
            $property,
            $entity
            $(, initialization_kind = $initialization_kind)?
            , is_required = true
            $(, collect_deps_fn = $collect_deps_fn)?
            $(, display_impl = $display_impl)?
            $(, canonical_value = $canonical_value)?
            $(, make_canonical = $make_canonical)?
            $(, make_uncanonical = $make_uncanonical)?
            $(, index_id_fn = $index_id_fn)?
            $(, ctor_registration = $ctor_registration)?
        );
    };

    // Case 4: none of the three conditions are met => hard error
    (
        $property:ident,
        $entity:ident
        $(, initialization_kind = $initialization_kind:expr)?
        $(, is_required = $is_required:expr)?
        $(, compute_derived_fn = $compute_derived_fn:expr)?
        $(, collect_deps_fn = $collect_deps_fn:expr)?
        $(, default_const = $default_const:expr)?
        $(, display_impl = $display_impl:expr)?
        $(, canonical_value = $canonical_value:ty)?
        $(, make_canonical = $make_canonical:expr)?
        $(, make_uncanonical = $make_uncanonical:expr)?
        $(, index_id_fn = $index_id_fn:expr)?
        $(, ctor_registration = $ctor_registration:expr)?
    ) => {
        compile_error!(
            "impl_property_with_options!: you must supply at least one of: \
             `is_required = true`, `default_const = ...`, or `compute_derived_fn = ...`."
        );
    };

    // Shared implementation
    (@impl
        $property:ident,
        $entity:ident
        $(, initialization_kind = $initialization_kind:expr)?
        $(, is_required = $is_required:expr)?
        $(, compute_derived_fn = $compute_derived_fn:expr)?
        $(, collect_deps_fn = $collect_deps_fn:expr)?
        $(, default_const = $default_const:expr)?
        $(, display_impl = $display_impl:expr)?
        $(, canonical_value = $canonical_value:ty)?
        $(, make_canonical = $make_canonical:expr)?
        $(, make_uncanonical = $make_uncanonical:expr)?
        $(, index_id_fn = $index_id_fn:expr)?
        $(, ctor_registration = $ctor_registration:expr)?
    ) => {
        $crate::__impl_property_common!(
            $property,
            $entity,

            // canonical value
            $crate::impl_property_with_options!(@unwrap_or_ty $($canonical_value)?, $property),

            // initialization_kind
            $crate::impl_property_with_options!(@unwrap_or
                $($initialization_kind)?,
                $crate::impl_property_with_options!(@unwrap_or_default_kind $($default_const)?)
            ),

            // is_required
            $crate::impl_property_with_options!(@unwrap_or $($is_required)?, false),

            // compute_derived_fn
            $crate::impl_property_with_options!(
                @unwrap_or
                $($compute_derived_fn)?,
                |_, _| panic!("property {} is not derived", stringify!($property))
            ),

            // collect_deps_fn
            $crate::impl_property_with_options!(
                @unwrap_or
                $($collect_deps_fn)?,
                |_| {/* Do nothing */}
            ),

            // default_const
            $crate::impl_property_with_options!(
                @unwrap_or
                $($default_const)?,
                panic!("property {} has no default value", stringify!($property))
            ),

            // display_impl
            $crate::impl_property_with_options!(@unwrap_or $($display_impl)?, |v| format!("{v:?}")),

            // make_canonical
            $crate::impl_property_with_options!(@unwrap_or $($make_canonical)?, std::convert::identity),

            // make_uncanonical
            $crate::impl_property_with_options!(@unwrap_or $($make_uncanonical)?, std::convert::identity),

            // index_id_fn
            $crate::impl_property_with_options!(@unwrap_or $($index_id_fn)?, {
                Self::id()
            }),

            // ctor_registration
            $crate::impl_property_with_options!(@unwrap_or $($ctor_registration)?, {
                $crate::entity::property_store::add_to_property_registry::<$entity, $property>();
            }),
        );
    };

    // Helpers for defaults, a pair per macro parameter type (`expr`, `ty`).
    (@unwrap_or $value:expr, $_default:expr) => { $value };
    (@unwrap_or, $default:expr) => { $default };

    (@unwrap_or_ty $ty:ty, $_default:ty) => { $ty };
    (@unwrap_or_ty, $default:ty) => { $default };

    (@unwrap_or_default_kind $expr:expr) => {
        $crate::entity::property::PropertyInitializationKind::Constant
    };
    (@unwrap_or_default_kind) => {
        $crate::entity::property::PropertyInitializationKind::Explicit
    };
}
pub use impl_property_with_options;

/// Internal macro used to define common boilerplate for property types that
/// implement the [`Property`] trait. The `impl_property_with_options`
/// macro provides a more ergonomic interface for this macro.
///
/// # Parameters
///
/// * `$property` — The name of the concrete type implementing [`Property`].
/// * `$entity` — The entity type this property is associated with.
/// * `$canonical_value` — The canonical type stored in the index if it differs
///   from the property’s own value type.
/// * `$initialization_kind` — The [`PropertyInitializationKind`] describing how
///   this property is initialized (e.g. `Constant`, `Dynamic`, `Derived`, etc.).
/// * `$is_required` — A boolean indicating whether new entities must have this
///   property explicitly set at creation time.
/// * `$compute_derived_fn` — A function or closure used to compute the property’s
///   value if it is derived from other properties.
/// * `$default_const` — The constant default value if the property has one.
/// * `$display_impl` — A function that takes a canonical value and returns a
///   string representation of the property.
/// * `$make_canonical` — A function that takes a `Self` and converts it to a `Self::CanonicalValue`.
/// * `$make_uncanonical` — A function that takes a `Self::CanonicalValue` and converts it to a `Self`.
/// * `$index_id_fn` — Code that returns the unique index for this property.
/// * `$ctor_registration` — Code run in the `ctor`.
#[macro_export]
macro_rules! __impl_property_common {
    (
        $property:ident,           // The name of the type we are implementing `Property` for
        $entity:ident,             // The entity type this property is associated with
        $canonical_value:ty,       // If the type stored in the index is different from Self, the name of that type
        $initialization_kind:expr, // The kind of initialization this property has
        $is_required:expr,         // Do we require that new entities have this property explicitly set?
        $compute_derived_fn:expr,  // If the property is derived, the function that computes the value
        $collect_deps_fn:expr,     // If the property is derived, the function that computes the value
        $default_const:expr,       // If the property has a constant default initial value, the default value
        $display_impl:expr,        // A function that takes a canonical value and returns a string representation of this property
        $make_canonical:expr,      // A function that takes a value and returns a canonical value
        $make_uncanonical:expr,    // A function that takes a canonical value and returns a value
        $index_id_fn:expr,            // Code that returns the unique index for this property
        $ctor_registration:expr,   // Code that runs in a ctor for property registration
    ) => {
        impl $crate::entity::property::Property<$entity> for $property {
            type CanonicalValue = $canonical_value;

            fn initialization_kind() -> $crate::entity::property::PropertyInitializationKind {
                $initialization_kind
            }

            fn is_required() -> bool {
                $is_required
            }

            fn compute_derived(
                _context: &$crate::Context,
                _entity_id: $crate::entity::EntityId<$entity>,
            ) -> Self {
                ($compute_derived_fn)(_context, _entity_id)
            }

            fn collect_non_derived_dependencies(result: &mut $crate::HashSet<usize>) {
                $collect_deps_fn(result)
            }

            fn default_const() -> Self {
                $default_const
            }

            fn make_canonical(self) -> Self::CanonicalValue {
                $make_canonical(self)
            }

            fn make_uncanonical(value: Self::CanonicalValue) -> Self {
                $make_uncanonical(value)
            }

            fn name() -> &'static str {
                stringify!($property)
            }

            fn get_display(&self) -> String {
                $display_impl(self)
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
                $crate::entity::property_store::initialize_property_index(&INDEX)
            }

            fn index_id() -> usize {
                $index_id_fn
            }
        }

        // Using `ctor` to initialize properties at program start-up means we know how many properties
        // there are at the time any `PropertyStore` is created, which means we never have
        // to mutate `PropertyStore` to initialize a `Property` that hasn't yet been accessed.
        // (The mutation happens inside of a `OnceCell`, which we can already have ready
        // when we construct `PropertyStore`.) In other words, we could do away with `ctor`
        // if we were willing to have a mechanism for interior mutability for `PropertyStore`.
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
pub use __impl_property_common;

/// An internal macro that expands to the correct implementation for the `compute_derived` function of a derived property.
#[macro_export]
macro_rules! __derived_property_compute_fn {
    (
        $entity:ident,
        [$($dependency:ident),*],
        [$($global_dependency:ident),*],
        |$($param:ident),+| $derive_fn:expr
    ) => {
        |context: &crate::Context, entity_id| {
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

/// The "derived" variant of [`define_property!`] for defining simple derived property types.
/// Defines a `struct` or `enum` with a standard set of derives and automatically invokes
/// [`impl_property!`] for it.
///
/// Defines a derived property with the following parameters:
/// * `$property`: A name for the identifier type of the property
/// * `$value`: The type of the property's value
/// * `[$($dependency),+]`: A list of person properties the derived property depends on
/// * `[$($dependency),*]`: A list of global properties the derived property depends on (optional)
/// * $calculate: A closure that takes the values of each dependency and returns the derived value
#[macro_export]
macro_rules! define_derived_property {
    // The calls to `$crate::impl_property_with_options!` are all the same except for
    // this first case of a newtype for an `Option<T>`, which has a special `display_impl`.

    // Struct (tuple) with single Option<T> field (special case)
    (
        struct $name:ident ( Option<$inner_ty:ty> ),
        $entity:ident,
        [$($dependency:ident),*],
        [$($global_dependency:ident),*],
        |$($param:ident),+| $derive_fn:expr
        // For `canonical_value` implementations:
        $(, $($extra:tt)+),*
    ) => {
        #[derive(Debug, PartialEq, Eq, Clone, Copy, $crate::serde::Serialize)]
        pub struct $name(Option<$inner_ty>);

        // Use impl_property_with_options! to provide a custom display implementation
        $crate::impl_property_with_options!(
            $name,
            $entity,
            initialization_kind = $crate::entity::property::PropertyInitializationKind::Derived,
            compute_derived_fn = $crate::__derived_property_compute_fn!(
                $entity,
                [$($dependency),*],
                [$($global_dependency),*],
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
            },
            display_impl = |value: &Option<$inner_ty>| {
                match value {
                    Some(v) => format!("{:?}", v),
                    None => "None".to_string(),
                }
            }
            $(, $($extra)+),*
        );
    };

    // Struct (tuple)
    (
        struct $name:ident ( $($field_ty:ty),* $(,)? ),
        $entity:ident,
        [$($dependency:ident),*],
        [$($global_dependency:ident),*],
        |$($param:ident),+| $derive_fn:expr
        // For `canonical_value` implementations:
        $(, $($extra:tt)+),*
    ) => {
        #[derive(Debug, PartialEq, Eq, Clone, Copy, $crate::serde::Serialize)]
        pub struct $name( $($field_ty),* );

        $crate::impl_property_with_options!(
            $name,
            $entity,
            initialization_kind = $crate::entity::property::PropertyInitializationKind::Derived,
            compute_derived_fn = $crate::__derived_property_compute_fn!(
                $entity,
                [$($dependency),*],
                [$($global_dependency),*],
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
            $(, $($extra)+),*
        );
    };

    // Struct (named fields)
    (
        struct $name:ident { $($field_name:ident : $field_ty:ty),* $(,)? },
        $entity:ident,
        [$($dependency:ident),*],
        [$($global_dependency:ident),*],
        |$($param:ident),+| $derive_fn:expr
        // For `canonical_value` implementations:
        $(, $($extra:tt)+),*
    ) => {
        #[derive(Debug, PartialEq, Eq, Clone, Copy, $crate::serde::Serialize)]
        pub struct $name { $($field_name : $field_ty),* }

        $crate::impl_property_with_options!(
            $name,
            $entity,
            initialization_kind = $crate::entity::property::PropertyInitializationKind::Derived,
            compute_derived_fn = $crate::__derived_property_compute_fn!(
                $entity,
                [$($dependency),*],
                [$($global_dependency),*],
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
            $(, $($extra)+),*
        );
    };

    // Enum without default
    (
        enum $name:ident {
            $($variant:ident),* $(,)?
        },
        $entity:ident,
        [$($dependency:ident),*],
        [$($global_dependency:ident),*],
        |$($param:ident),+| $derive_fn:expr
        // For `canonical_value` implementations:
        $(, $($extra:tt)+),*
    ) => {
        #[derive(Debug, PartialEq, Eq, Clone, Copy, $crate::serde::Serialize)]
        pub enum $name {
            $($variant),*
        }

        $crate::impl_property_with_options!(
            $name,
            $entity,
            initialization_kind = $crate::entity::property::PropertyInitializationKind::Derived,
            compute_derived_fn = $crate::__derived_property_compute_fn!(
                $entity,
                [$($dependency),*],
                [$($global_dependency),*],
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
            $(, $($extra)+),*
        );
    };
}
pub use define_derived_property;

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

                $crate::impl_property_with_options!(
                    [<$($dependency)*>],
                    $entity,
                    initialization_kind = $crate::entity::property::PropertyInitializationKind::Derived,
                    compute_derived_fn = |context: &$crate::Context, entity_id: $crate::entity::EntityId<$entity>| {
                        (
                            $(context.get_property::<$entity, $dependency>(entity_id)),+
                        )
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
                    make_canonical = reorder_closure!(( $($dependency),+ )),
                    make_uncanonical = unreorder_closure!(( $($dependency),+ )),

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
                        type_ids.sort();
                        // Check if an index has already been assigned to this property set.
                        match $crate::entity::multi_property::type_ids_to_multi_property_index(&type_ids) {
                            Some(index) => {
                                // An index exists. Reuse it for our own index.
                                INDEX_ID.store(index, std::sync::atomic::Ordering::Relaxed);
                                index
                            },
                            None => {
                                // An index ID is not yet assigned. We will use our own index for this property.
                                let index = Self::id();
                                INDEX_ID.store(index, std::sync::atomic::Ordering::Relaxed);
                                // And register the new index with this property set.
                                $crate::entity::multi_property::register_type_ids_to_muli_property_index(
                                    &type_ids,
                                    index
                                );
                                index
                            }
                        }
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
pub use define_multi_property;

#[cfg(test)]
mod tests {
    // We define unused properties to test macro implementation.
    #![allow(dead_code)]

    use ixa_derive::{reorder_closure, unreorder_closure};

    use crate::prelude::*;

    define_entity!(Person);
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
        [],    // No global dependencies
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
    define_derived_property!(struct DerivedProp(bool), Person, [Age], [], |age| {
        DerivedProp(age.0 % 2 == 0)
    });

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
        assert!(context
            .property_store
            .is_property_indexed::<Person, ProfileNAW>());
        assert!(context
            .property_store
            .is_property_indexed::<Person, ProfileAWN>());
        assert!(context
            .property_store
            .is_property_indexed::<Person, ProfileWAN>());
        // ...but only one `Index<E, P>` instance was created.
        let mut indexed_count = 0;
        if context
            .property_store
            .get::<_, ProfileNAW>()
            .index
            .is_some()
        {
            indexed_count += 1;
        }
        if context
            .property_store
            .get::<_, ProfileAWN>()
            .index
            .is_some()
        {
            indexed_count += 1;
        }
        if context
            .property_store
            .get::<_, ProfileWAN>()
            .index
            .is_some()
        {
            indexed_count += 1;
        }
        assert_eq!(indexed_count, 1);

        // ToDo(RobertJacobsonCDC): Uncomment the following when queries are implemented for entities.

        // {
        //     let example_query = (Name("Alice"), Age(22), Weight(170.5));
        //     let query_multi_property_type_id = Query::multi_property_type_id(&example_query);
        //     assert!(query_multi_property_type_id.is_some());
        //     assert_eq!(ProfileNAW::type_id(), query_multi_property_type_id.unwrap());
        //     assert_eq!(
        //         Query::multi_property_value_hash(&example_query),
        //         ProfileNAW::hash_property_value(&ProfileNAW(Name("Alice"), Age(22), Weight(170.5)).make_canonical())
        //     );
        // }
        //
        // context.with_query_results((ProfileNAW(Name("John"), Age(42), Weight(220.5)),), &mut |results| {
        //     assert_eq!(results.len(), 1);
        // });
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
        expected_dependents.sort();
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

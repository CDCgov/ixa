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
        #[derive(Debug, PartialEq, Eq, Clone, Copy, $crate::serde::Serialize)]
        pub struct $name(Option<$inner_ty>);

        // Use impl_property_with_options! to provide a custom display implementation
        $crate::impl_property_with_options!(
            $name,
            $entity
            $(, $($extra)+)*
            , display_impl = |value: &Option<$inner_ty>| {
                match value {
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
        #[derive(Debug, PartialEq, Eq, Clone, Copy, $crate::serde::Serialize)]
        pub struct $name($($field_ty),*);
        $crate::impl_property!($name, $entity $(, $($extra)+)*);
    };

    // Struct (named fields)
    (
        struct $name:ident { $($field_name:ident : $field_ty:ty),* $(,)? },
        $entity:ident
        $(, $($extra:tt)+),*
    ) => {
        #[derive(Debug, PartialEq, Eq, Clone, Copy, $crate::serde::Serialize)]
        pub struct $name { $($field_name : $field_ty),* }
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
        #[derive(Debug, PartialEq, Eq, Clone, Copy, $crate::serde::Serialize)]
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
    ) => {
        compile_error!(
            "impl_property_with_options!: you must supply at least one of: \
             `is_required = true`, `default_const = ...`, or `compute_derived_fn = ...`."
        );
    };

    // Shared implementation (this is essentially your original macro body)
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
            $crate::impl_property_with_options!(@unwrap_or $($make_uncanonical)?, std::convert::identity)
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
#[macro_export]
macro_rules! __impl_property_common {
    (
        $property:ident,           // The name of the type we are implementing `Property` for
        $entity:ident,             // The entity type this property is associated with
        $canonical_value:ty,       // If the type stored in the index is different from Self, the name of that type
        $initialization_kind:expr, // The kind of initialization this property has
        $is_required:expr,         // Do we require that new entities have this property explicitly set?
        $compute_derived_fn:expr,  // If the property is derived, the function that computes the value
        $collect_deps_fn:expr,  // If the property is derived, the function that computes the value
        $default_const:expr,       // If the property has a constant default initial value, the default value
        $display_impl:expr,         // A function that takes a canonical value and returns a string representation of this property
        $make_canonical:expr,      // A function that takes a value and returns a canonical value
        $make_uncanonical:expr     // A function that takes a canonical value and returns a value
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
            ) -> Self::CanonicalValue {
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

            fn index() -> usize {
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
                    $crate::entity::property_store::add_to_property_registry::<$entity, $property>();
                }
            }
        }
    };
}
pub use __impl_property_common;

/*
/// Defines a derived property with the following parameters:
/// * `$property`: A name for the identifier type of the property
/// * `$value`: The type of the property's value
/// * `[$($dependency),+]`: A list of person properties the derived property depends on
/// * `[$($dependency),*]`: A list of global properties the derived property depends on (optional)
/// * `$calculate`: A closure that takes the values of each dependency and returns the derived value
/// * `$display`: A closure that takes the value of the derived property and returns a string representation
/// * `$hash_fn`: A function that can compute the hash of values of this property
#[macro_export]
macro_rules! __define_derived_property_common {
    (
        $derived_property:ident,
        $entity:ty,
        $value:ty,
        $canonical_value:ty,
        $compute_canonical_impl:expr,
        $compute_uncanonical_impl:expr,

        $at_dependency_registration:expr,

        [$($dependency:ident),*],
        [$($global_dependency:ident),*],

        |$($param:ident),+| $derive_fn:expr,

        $display_impl:expr,

        $hash_fn:expr,

        $type_id_impl:expr
    ) => {
        #[derive(Debug, Copy, Clone)]
        pub struct $derived_property;

        impl $crate::people::Property for $derived_property {
            type Value = $value;
            type CanonicalValue = $canonical_value;

            fn initialization_kind() -> $crate::people::PropertyInitializationKind {
                $crate::people::PropertyInitializationKind::Derived
            }

            fn compute(context: &$crate::context::Context, person_id: $crate::people::PersonId) -> Self::Value {
                #[allow(unused_imports)]
                use $crate::global_properties::ContextGlobalPropertiesExt;
                #[allow(unused_parens)]
                let ($($param,)*) = (
                    $(context.get_property(person_id, $dependency)),*,
                    $(
                        context.get_global_property_value($global_dependency)
                            .expect(&format!("Global property {} not initialized", stringify!($global_dependency)))
                    ),*
                );
                #[allow(non_snake_case)]
                (|$($param),+| $derive_fn)($($param),+)
            }

            fn compute_immutable(context: &$crate::context::Context, person_id: $crate::people::PersonId) -> Self::Value {
                #[allow(unused_imports)]
                use $crate::global_properties::ContextGlobalPropertiesExt;
                #[allow(unused_parens)]
                let ($($param,)*) = (
                    $(context.get_property_immutable(person_id, $dependency)),*,
                    $(
                        // Right now `get_global_property_value` is always an immutable operation.
                        context.get_global_property_value($global_dependency)
                            .expect(&format!("Global property {} not initialized", stringify!($global_dependency)))
                    ),*
                );
                #[allow(non_snake_case)]
                (|$($param),+| $derive_fn)($($param),+)
            }

            fn make_canonical(value: Self::Value) -> Self::CanonicalValue {
                ($compute_canonical_impl)(value)
            }
            fn make_uncanonical(value: Self::CanonicalValue) -> Self::Value {
                ($compute_uncanonical_impl)(value)
            }
            fn is_derived() -> bool { true }
            fn dependencies() -> Vec<Box<dyn $crate::people::PropertyHolder>> {
                vec![$(
                    Box::new($dependency) as Box<dyn $crate::people::PropertyHolder>
                ),*]
            }
            fn register_dependencies(context: &$crate::context::Context) {
                $at_dependency_registration
                $(context.register_property::<$dependency>();)+
            }
            fn get_instance() -> Self {
                $derived_property
            }
            fn name() -> &'static str {
                stringify!($derived_property)
            }
            fn get_display(value: &Self::CanonicalValue) -> String {
                $display_impl(value)
            }
            fn hash_property_value(value: &Self::CanonicalValue) -> u128 {
                ($hash_fn)(value)
            }
            fn type_id() -> std::any::TypeId {
                $type_id_impl
            }
        }
    };
}
*/

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
            #[allow(non_snake_case)]
            (|$($param),+| $derive_fn)($($param),+)
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
                        deps.insert($dependency::index());
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
        pub struct $name(Option<$inner_ty>);

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
                        deps.insert($dependency::index());
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
                        deps.insert($dependency::index());
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
                        deps.insert($dependency::index());
                    }
                )*
            }
            $(, $($extra)+),*
        );
    };
}
pub use define_derived_property;

/*
#[macro_export]
macro_rules! define_multi_property {
    (
        $property:ident,
        ( $($dependency:ident),+ )
    ) => {
        // $crate::sorted_property_impl!(( $($dependency),+ ));
        $crate::paste::paste! {
            $crate::__define_derived_property_common!(
                // Name
                $property,

                // `Property::Value` type
                ( $(<$dependency as $crate::people::Property>::Value),+ ),

                // `Property::CanonicalValue` type
                $crate::sorted_value_type!(( $($dependency),+ )),

                // Function to transform a `Property::Value` to a `Property::CanonicalValue`
                $property::reorder_by_tag,

                // Function to transform a `Property::CanonicalValue` to a `Property::Value`
                $property::unreorder_by_tag,

                // Code that runs at dependency registration time
                {
                    let type_ids = &mut [$($dependency::type_id()),+ ];
                    type_ids.sort();
                    $crate::people::register_type_ids_to_multi_property_id(type_ids, Self::type_id());
                },

                // Property dependency list
                [$($dependency),+],

                // Global property dependency list
                [],

                // A function that takes the values of each dependency and returns the derived value
                |$( [<_ $dependency:lower>] ),+| {
                    ( $( [<_ $dependency:lower>] ),+ )
                },

                // A function that takes a canonical value and returns a string representation of it.
                |values_tuple: &Self::CanonicalValue| {
                    // ice tThe string representation uses the original (unsorted) ordering.
                    let values_tuple: Self::Value = Self::unreorder_by_tag(*values_tuple);
                    let mut displayed = String::from("(");
                    let ( $( [<_ $dependency:lower>] ),+ ) = values_tuple;
                    $(
                        displayed.push_str(<$dependency as $crate::Property>::get_display(
                            & <$dependency as $crate::Property>::make_canonical([<_ $dependency:lower>])
                        ).as_str());
                        displayed.push_str(", ");
                    )+
                    displayed.truncate(displayed.len() - 2);
                    displayed.push_str(")");
                    displayed
                },

                // A function that computes the hash of a value of this property
                $crate::hashing::hash_serialized_128,

                // The Type ID of the property.
                // The type ID of a multi-property is the type ID of the SORTED tuple of its
                // components. This is so that tuples with the same component types in a different
                // order will have the same type ID.
                std::any::TypeId::of::<$crate::sorted_tag!(( $($dependency),+ ))>()
            );
            $crate::impl_make_canonical!($property, ( $($dependency),+ ));
        }
    };
}
pub use define_multi_property;
*/

#[cfg(test)]
mod tests {
    use crate::define_entity;
    use crate::entity::property::Property;
    use crate::prelude::*;

    define_entity!(Person);

    define_property!(
        struct Age(u8),
        Person,
        is_required = true
    );

    // An enum
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

        println!("{}.index = {}", Age::name(), Age::index());
        println!("{}.index = {}", AgeGroup::name(), AgeGroup::index());
        println!(
            "{}.dependencies = {:?}",
            Age::name(),
            Age::non_derived_dependencies()
        );
        println!(
            "{}.dependencies = {:?}",
            AgeGroup::name(),
            AgeGroup::non_derived_dependencies()
        );
        println!("{}.dependents = {:?}", Age::name(), Age::dependents());
        println!(
            "{}.dependents = {:?}",
            AgeGroup::name(),
            AgeGroup::dependents()
        );
    }
}

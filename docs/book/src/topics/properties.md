# Properties

Properties are the data attached to an entity. For example, a `Person` entity might have the properties `Age` and
`InfectionStatus`. In Ixa, the property's value type is also the property type: the implementation of the trait
`Property\<Person>` by the concrete property type is what ties that Rust type to the `Person` entity.

Never implement `Property\<E>` for an `Entity` type `E` directly. Instead, use one of the provided macros:

| Macro                      | Use case                                      |
|:---------------------------|:----------------------------------------------|
| `define_property!`         | Simple struct or enum property                |
| `impl_property!`           | Existing type                                 |
| `define_derived_property!` | Simple derived struct or enum property        |
| `impl_derived_property!`   | Existing type as a derived property           |
| `define_multi_property!`   | Joint index/query key for multiple properties |

## Properties Basics

### Defining Properties

Most model code should define properties with `define_property!`:

```rust
use ixa::prelude::*;

define_entity!(Person);

define_property!(struct Age(u8), Person);

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

The first argument is a Rust type declaration. The second argument is the entity the property belongs to. The optional
`default_const = ...` argument gives the property a constant value for new entities that do not provide one explicitly.
Without it, every call to `add_entity` requires a value for the property.

More advanced use cases and options are covered in the sections below.

> [!INFO] The newtype idiom in Rust
>
> For properties whose values are essentially [primitive types](https://doc.rust-lang.org/rust-by-example/primitives.html) like `bool` or `u64`, we always use the [newtype idiom](https://doc.rust-lang.org/rust-by-example/generics/new_types.html). A "newtype" is a tuple struct with a single field that wraps an existing type to give it a new, distinct identity:
>
> ```rust
> struct Age(u8);
> ```
>
> Even though `Age` is "just" a `u8` under the hood, the Rust compiler treats `Age` and `u8` as completely different
> types. You cannot accidentally pass a `u8` where an `Age` is expected, nor mix up an `Age` with some other newtype like
> `struct BirthYear(u8)`. This is exactly what we want for properties: in Ixa, each property is identified by its Rust
> type, so `Age` and `BirthYear` must be distinct types even when they happen to wrap the same primitive.
>
> To read the inner value out of a newtype, use the tuple field accessor `.0`. To produce a new value, wrap a primitive
> with the type's constructor:
>
> ```rust
> let age: Age = context.get_property(person_id);
> let new_age = Age(age.0 + 1);   // unwrap with `.0`, do arithmetic, re-wrap
> context.set_property(person_id, new_age);
> ```
>
> If unwrapping and re-wrapping becomes tedious, you can implement methods or operator traits like `Add` directly on
> your newtype so that model code can work with it more naturally. For a more thorough treatment of newtypes and what
> they're good for, see the chapter on
> [advanced types](https://doc.rust-lang.org/book/ch20-03-advanced-types.html#using-the-newtype-pattern-for-type-safety-and-abstraction)
> in *The Rust Book*.

### Property Initialization

Every property of an entity instance must have a value. (See the section [Optional Properties](#optional-properties) for how to deal with
properties that are not always present.) How a property is initialized depends on how the property is defined and on
the property values supplied to `add_entity`.
Every property has one of three initialization behaviors:

| Kind       | How to define it                       | How new entities get a value                                  |
| ---------- | -------------------------------------- | ------------------------------------------------------------- |
| Explicit   | Omit `default_const = ...`             | The value must be supplied to `add_entity` with `with!`       |
| Constant   | Add `default_const = ...`              | The default is used unless a value is supplied with `with!`   |
| Derived    | Use `define_derived_property!`         | The value is computed from other properties and cannot be set |

Derived properties are covered in the section [Derived Properties](#derived-properties). They are computed from other
properties by definition, so it doesn't make sense to initialize them explicitly.

An explicit property must be provided when the entity is created:

```rust
define_property!(struct Age(u8), Person);

let person = context.add_entity(with!(Person, Age(42)))?;
```

A property with a default constant can be omitted at entity creation:

```rust
define_property!(
    enum InfectionStatus {
        Susceptible,
        Infectious,
        Recovered,
    },
    Person,
    default_const = InfectionStatus::Susceptible
);

let person = context.add_entity(Person)?;
```

You can still override a property's initial value constant:

```rust
let person = context.add_entity(with!(Person, InfectionStatus::Infectious))?;
```

The word "constant" refers to the fact that the default value is not itself computed—it is a static value provided in
the property definition. The property itself, however, can still be overwritten after the fact with
`context.set_property` regardless of how it was initialized.

### Getting and Setting Properties

Once an entity exists, use `get_property` and `set_property`:

```rust
// The person with ID `person_id` had a birthday.
let age: Age = context.get_property(person_id);
context.set_property(person_id, Age(age.0 + 1));
```

Derived properties are computed from their dependencies and update when those dependencies change. Therefore, you
cannot set a derived property directly with `set_property`.

You can also query by property values with `with!`:

```rust
// Get the set of all people who are infectious.
let infectious = context.query_result_iterator(with!(
    Person,
    InfectionStatus::Infectious
));
```

For performance-sensitive queries, see the chapter on
[Indexing](indexing.md).

### Optional Properties

Sometimes you want a property that does not always have a meaningful value.
The idiomatic way to do this is to use a type of the form `struct MyProperty(Option\<ValueType>)`.
For example, you might store the time of a person's last vaccination
as `struct LastVaccination(Option\<f64>)`.

```rust
define_property!(
    struct LastVaccination(Option<f64>),
    Person,
    impl_eq_hash = neither,
    default_const = LastVaccination(None)
);
```

Notice that we provided a default of `LastVaccination(None)`, which stands for "no value set". Because this example
contains an `f64` and does not need to be indexed, we also pass `impl_eq_hash = neither` so `define_property!` does not
try to derive `Eq` and `Hash`. No manual `Eq` or `Hash` implementations are required for unindexed floating-point
properties.
See [Floating Point Types and Implementing `Eq` and `Hash`](#floating-point-types-and-implementing-eq-and-hash) for
the cases where `Eq` and `Hash` are still needed.

This is such a common pattern that Ixa detects the `Option` and provides a custom "display function" for it
for writing values to reports and diagnostics; see the section on
[Custom Display and Option Properties](#custom-display-and-option-properties).

## Custom Display and Option Properties

Ixa uses each property's display implementation in places where a property value
is rendered as text, including reports and diagnostics. By default,
`impl_property!` uses the property's `Debug` representation.

> [!INFO] The `Display` and `Debug` traits in Rust
>
> Rust types implement the `Display` and `Debug` traits to provide a textual representation of the type. `Debug` can be
> automatically `derive`d, shows a value in a developer-focused way, and is typically used for things like error
> messages or logs. `Display` shows a value in a user-facing way. You write it yourself to produce clean, readable
> output.
>
> In the context of Ixa properties, a property's "display function" is the function that is used to render a property
> for reports. We use the type's `Debug` representation by default because it can be automatically `derive`d for most
> types. Properties do not need to implement `Display` even if you supply a custom display function, although it can be
> useful to do so.

The `define_property!` macro automatically detects the case of a struct wrapping
a single `Option\<T>` field and provides a custom display function:

```rust
define_property!(
    struct DiagnosisDay(Option<u32>),
    Person,
    default_const = DiagnosisDay(None)
);
```

Without this special treatment, the values would display as:

| Value                    | Display                    |
| :----------------------- | :------------------------- |
| `DiagnosisDay(Some(14))` | `"DiagnosisDay(Some(14))"` |
| `DiagnosisDay(None)`     | `"DiagnosisDay(None)"`     |

The macro overrides this to display the values as:

| Value                    | Display  |
| :----------------------- | :------- |
| `DiagnosisDay(Some(14))` | `"14"`   |
| `DiagnosisDay(None)`     | `"None"` |

You can supply your own display function override for any property form using
`define_property!` as follows:

```rust
define_property!(
    enum InfectionStatus {
        Susceptible,
        Infectious,
        Recovered,
    },
    Person,
    default_const = InfectionStatus::Susceptible,
    display_impl = |status: &InfectionStatus| match status {
        InfectionStatus::Susceptible => "S".to_string(),
        InfectionStatus::Infectious => "I".to_string(),
        InfectionStatus::Recovered => "R".to_string(),
    }
);
```

Since the `display_impl` argument is attached to the `impl Property\<Entity> for ConcretePropertyType`, a single
`ConcretePropertyType` can have different display functions for different entities for which it is a property.

## When to Use `impl_property!`

Start with `define_property!` when the property can be expressed as one of its
supported type forms: a tuple struct, a named-field struct, or a simple enum:

```rust
define_property!(struct Age(u8), Person);

define_property!(
    struct Location {
        county: u16,
        tract: u32,
    },
    Person
);

define_property!(
    enum RiskGroup {
        Low,
        Medium,
        High,
    },
    Person
);
```

For these forms, `define_property!` creates the type, makes it public, adds the
standard derives, and then calls `impl_property!` for you. By default, the
standard derives are `Debug`, `PartialEq`, `Eq`, `Hash`, `Clone`, `Copy`,
`serde::Serialize`, and `serde::Deserialize`. The default includes `Eq` and
`Hash` so the property can be indexed or used as a hash-map key without more
boilerplate, but ordinary unindexed properties do not require those two traits.
Use the `impl_eq_hash = ...` argument when the default `Eq`/`Hash` derives are
not possible or not wanted.

Use `impl_property!` when the type already exists or when the type declaration
needs syntax that `define_property!` does not support. The common reasons are:

- You require a different set of derives than `define_property!` generates.
- You need an unsupported type form, such as a type with attributes, generic
  parameters, or more complex Rust syntax.
- You want to use the same Rust type as a property for multiple different
  entity types.

When you use `impl_property!`, you are responsible for making sure the type
implements the traits Ixa requires for all properties: `Copy`, `Clone`,
`Debug`, and `PartialEq`. If you want to index the property with
`context.index_property` or `context.index_property_counts`, it must also
implement `Eq` and `Hash`.

If a manually declared property needs `Eq` or `Hash` and those traits cannot be
derived, use `impl_property_eq!`, `impl_property_hash!`, or
`impl_property_eq_hash!` to generate the same byte-based implementations used by
`define_property!(..., impl_eq_hash = ...)`. This most often comes up for
indexed properties containing `f32` or `f64`. The type must derive
`ixa::rkyv::Archive` and `ixa::rkyv::Serialize` for these macros.

### Example: You require different derives

If a property type needs derives beyond the standard set generated by
`define_property!`, define the type yourself and then use `impl_property!`.
For example, a property loaded from external data may need `Deserialize`:

```rust
#[derive(
    Copy,
    Clone,
    Debug,
    PartialEq,
    serde::Serialize,
    serde::Deserialize,
)]
pub struct Age(pub u8);

impl_property!(Age, Person);
```

This is enough for an ordinary property. Add or implement `Eq` and `Hash` as
well if this property will be indexed or used as a key in a hash map.

### Example: You need an unsupported type form

`define_property!` intentionally supports a small set of common type
declarations. If the declaration needs field attributes, variant attributes,
generics, non-public fields, or other syntax outside those forms, write the type
directly.

For example, `define_property!` cannot attach a field-level serde attribute:

```rust
#[derive(Copy, Clone, Debug, PartialEq, serde::Serialize)]
pub struct HouseholdCode {
    #[serde(rename = "household")]
    pub value: u32,
}

impl_property!(
    HouseholdCode,
    Person,
    default_const = HouseholdCode { value: 0 }
);
```

Another common example is an enum that needs `Default`, because Rust requires
the default variant to be marked with `#[default]`:

```rust
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub enum InfectionStatus {
    #[default]
    Susceptible,
    Infectious,
    Recovered,
}

impl_property!(
    InfectionStatus,
    Person,
    default_const = InfectionStatus::Susceptible
);
```

### Example: You want the same type on multiple entities

`define_property!` defines a new type and implements it as a property for one
entity. If you want the same Rust type to be a property of more than one entity,
use `define_property!` for the first entity, then call `impl_property!` for
each additional entity:

```rust
define_entity!(Person);
define_entity!(Group);

define_property!(
    enum InfectionKind {
        Respiratory,
        Genetic,
    },
    Person,
    default_const = InfectionKind::Respiratory
);

impl_property!(
    InfectionKind,
    Group,
    default_const = InfectionKind::Genetic
);
```

## Derived Properties

A derived property is computed from other properties instead of stored directly.
Use `define_derived_property!` for the common case:

```rust
define_property!(struct Age(u8), Person);

define_derived_property!(
    enum AgeGroup {
        Child,
        Adult,
        Senior,
    },
    Person,
    [Age],
    |age| {
        if age.0 < 18 {
            AgeGroup::Child
        } else if age.0 < 65 {
            AgeGroup::Adult
        } else {
            AgeGroup::Senior
        }
    }
);
```

The dependency list tells Ixa which stored properties affect the derived value.
When one of those dependencies changes, Ixa can update indexes and property
change events for the derived property.

Derived properties can depend on more than one property:

```rust
define_property!(struct Vaccinated(bool), Person, default_const = Vaccinated(false));

define_derived_property!(
    struct HighPriority(bool),
    Person,
    [Age, Vaccinated],
    |age, vaccinated| HighPriority(age.0 >= 65 && !vaccinated.0)
);
```

Derived properties can also depend on global properties. Put global dependencies
in a second bracketed list after the entity-property dependencies:

```rust
define_global_property!(AdultAge, u8);

define_derived_property!(
    struct IsAdult(bool),
    Person,
    [Age],
    [AdultAge],
    |age, adult_age| IsAdult(age.0 >= *adult_age)
);
```

Use `impl_derived_property!` when the derived property type already exists, just
as you would use `impl_property!` instead of `define_property!` for a
non-derived property.

The same trait rule applies to derived properties: all derived properties need
`Copy`, `Clone`, `Debug`, and `PartialEq`; derived properties that are indexed
also need `Eq` and `Hash`. The `define_derived_property!` macro includes
`Eq` and `Hash` in its default derives, just like `define_property!`, unless
you pass `impl_eq_hash = ...`.

## Floating Point Types and Implementing `Eq` and `Hash`

All properties participate in equality for unindexed query scans, so every property needs `PartialEq`. Indexed
properties have the additional requirements of `Eq` and `Hash` because property indexes are hash maps keyed by property
value.

Plain `f64` and `f32` values implement `PartialEq`, so they can be used in
ordinary unindexed properties. They do not implement `Eq` or `Hash`, so a
floating-point property needs special handling only when:

- you use the default `define_property!` or `define_derived_property!` form,
  which tries to derive `Eq` and `Hash` unless told otherwise;
- you index the property with `context.index_property` or
  `context.index_property_counts`;
- you use the property as part of an indexed multi-property; or
- you use the property type as a key in a `HashMap` or `HashSet`.

> [!INFO] Implementing `PartialEq` and `Eq` in Rust
>
> Indexed properties need to implement `Eq`. In practice, this actually means implementing `PartialEq`. In fact, the `Eq` trait
> is just a marker trait; it has no methods! The `Eq` trait is a guarantee by the author that the implementation of
> `PartialEq` [is reflexive](https://doc.rust-lang.org/std/cmp/trait.Eq.html). Rust also requires that `PartialEq`
> is symmetric and transitive.

### Unindexed floating-point properties

If a floating-point property will not be indexed and will not be used as a hash
map key, pass `impl_eq_hash = neither` to `define_property!` or
`define_derived_property!`:

```rust
define_property!(
    struct Weight(f64),
    Person,
    impl_eq_hash = neither,
    default_const = Weight(0.0)
);
```

That tells the macro not to derive or generate `Eq` and `Hash`. No manual `Eq`
or `Hash` implementations are required for this unindexed case.

If you define the type yourself and call `impl_property!` or
`impl_derived_property!`, derive or implement the ordinary property traits:
`Copy`, `Clone`, `Debug`, and `PartialEq`. Do not add `Eq` and `Hash` unless the
property needs to be indexed or used as a key.

### Indexed or hash-keyed floating-point properties

If a floating-point property is indexed, is part of an indexed multi-property,
or is used as a hash-map key, it must implement `Eq` and `Hash`. There are three
reasonable ways to do that.

1. [Let Ixa generate equality and hashing for you](#let-ixa-generate-equality-and-hashing).
2. [Implement equality and hashing yourself](#implement-equality-and-hashing-yourself).
3. [Use an alternative floating-point type](#use-an-alternative-floating-point-type).

### Let Ixa generate equality and hashing

Ixa can generate byte-based equality and hashing either when it defines the
property type for you or when you declare the type yourself and register it with
`impl_property!` or `impl_derived_property!`.

#### Types declared with `define_property!` or `define_derived_property!`

Pass `impl_eq_hash = both` as the first optional argument to
`define_property!` or `define_derived_property!`:

```rust
define_property!(
    struct Weight(f64),
    Person,
    impl_eq_hash = both,
    default_const = Weight(0.0)
);
```

This is the shortest option when you define the property with a `define_*`
macro, want to keep the property as an `f64`, and are comfortable with Ixa's
generated equality and hashing behavior. It is a good fit for simple measured
quantities where model code still wants direct access to a floating-point value.

You can also use only `impl_eq_hash = Eq` or only `impl_eq_hash = Hash` when
one trait can still be derived but the other needs Ixa's generated
implementation. Floating-point properties usually need `both`.

The generated implementations are reasonably efficient, but they are not optimal.
If performance is absolutely critical, use either of the other options.

#### Manually declared types registered with `impl_*_property!`

For a type declared manually and registered with `impl_property!` or
`impl_derived_property!`, use the standalone equality and hashing macros instead
of the `impl_eq_hash` parameter.

| Macro                    | Purpose                                      |
|:-------------------------|:---------------------------------------------|
| `impl_property_eq!`      | Implements `PartialEq` and `Eq`              |
| `impl_property_hash!`    | Implements `Hash`                            |
| `impl_property_eq_hash!` | Convenience macro that invokes both macros   |

```rust
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
struct Weight(f64);

impl_property_eq_hash!(Weight);
impl_property!(Weight, Person, default_const = Weight(0.0));
```

Use `impl_property_eq!` or `impl_property_hash!` when you only want Ixa to
generate one of the two implementations. These macros are only needed when the
property actually needs the corresponding trait, such as when it will be indexed
or used as a key. Manually declared types using these macros must derive
`ixa::rkyv::Archive` and `ixa::rkyv::Serialize`, as shown above.

### Implement equality and hashing yourself

Pass `impl_eq_hash = neither` when you want `define_property!` to create the
type but you want to provide `PartialEq`, `Eq`, and `Hash` yourself:

```rust
use std::hash::{Hash, Hasher};

define_property!(
    struct Weight(f64),
    Person,
    impl_eq_hash = neither,
    default_const = Weight(0.0)
);

impl PartialEq for Weight {
    fn eq(&self, other: &Self) -> bool {
        self.0.to_bits() == other.0.to_bits()
    }
}

impl Eq for Weight {}

impl Hash for Weight {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.to_bits().hash(state);
    }
}
```

Choose this option when you want to use `f32` / `f64` instead of `OrderedFloat` or equivalent alternative float type
and performance is absolutely crucial. It is usually straightforward to implement `PartialEq` and `Hash` for
floating-point types, but you should ensure that your implementations satisfy the property that equal values have the
same hash:

```rust
if a == b {
    assert_eq!(hash(a), hash(b));
}
```

For sensible equality semantics, equality should also be reflexive, symmetric, and transitive as well.

### Use an alternative floating-point type

Use a wrapper type from a crate such as
[`decorum`](https://crates.io/crates/decorum) or
[`ordered-float`](https://crates.io/crates/ordered-float) when you want a type
that gives floating-point values a total ordering and implements the traits Ixa
needs:

```rust
use ordered_float::OrderedFloat;

// A type alias is always a good idea here. It allows you to swap out the underlying
// type without having to change the rest of your code.
pub type Float = OrderedFloat<f64>;

define_property!(
    struct Weight(Float),
    Person,
    default_const = Weight(OrderedFloat(0.0))
);
```

With these types, `define_property!` can just `derive` the `PartialEq`, `Eq`,
and `Hash` traits that indexed or hash-keyed properties need, and performance
of these derived implementations is usually optimal. This option is especially
attractive when you want to restrict your type only to the real numbers, or only
to the extended real numbers (infinities but not NaNs), giving you a numeric
type with exactly the mathematical semantics you want.

The trade-off is that model code works with the wrapper type instead of a bare `f64`, and while these libraries do what
they can to alleviate friction, having to convert to and from primitive `f64` values is often unavoidable. This is
really the only downside. The good news is, this conversion is usually only cosmetic: the compiler usually optimizes it
away.

## Multi-Properties

A multi-property is a derived tuple of several properties. Its main purpose is
to support efficient multi-property indexes and queries:

```rust
define_property!(struct Age(u8), Person);
define_property!(
    enum InfectionStatus {
        Susceptible,
        Infectious,
        Recovered,
    },
    Person,
    default_const = InfectionStatus::Susceptible
);

define_multi_property!(Person, (Age, InfectionStatus));
```

By default, this creates a full multi-property index. You can pass an explicit
`ixa::entity::PropertyIndexType` as a third argument to request a count-only
index or to opt out of automatic indexing:

```rust
define_multi_property!(
    Person,
    (Age, InfectionStatus),
    ixa::entity::PropertyIndexType::Unindexed
);
```

Use the underlying property names in `define_multi_property!`, not type aliases. For a deeper discussion of when to
create multi-property indexes, see [Indexing](indexing.md). Multi-property component properties are not individually
indexed unless you index them separately. Because the components of a multi-property are already required to be
properties, multi-properties usually "just work". Each component value must also support `Eq` and `Hash`; this mainly
matters for components containing plain `f32` or `f64`.

## Troubleshooting

### `f64` does not implement `Eq`

If you define a property containing an `f64` with the default macro form:

```rust
define_property!(struct Weight(f64), Person);
```

you may see an error like:

```text
error[E0277]: the trait bound `f64: std::cmp::Eq` is not satisfied
```

or:

```text
the trait `std::cmp::Eq` is not implemented for `f64`
```

This happens because `define_property!` normally derives `Eq` and `Hash`, but
Rust floating-point types do not implement those traits. See
[Floating Point Types and Implementing `Eq` and `Hash`](#floating-point-types-and-implementing-eq-and-hash)
for the full discussion. In short:

- If the property is not indexed and is not used as a hash-map key, pass
  `impl_eq_hash = neither`. No manual `Eq` or `Hash` implementations are
  required.
- If the property is indexed or used as a key, pass `impl_eq_hash = both` to let
  Ixa generate equality and hashing, provide your own implementations with
  `impl_eq_hash = neither`, or use a wrapper such as
  `ordered_float::OrderedFloat\<f64>` or a type from `decorum`.

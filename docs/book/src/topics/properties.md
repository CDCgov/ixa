# Properties

Properties are the data attached to an entity. For example, a `Person` entity might have the properties `Age` and
`InfectionStatus`. In ixa, the property's value type is also the property type: the implementation of the trait
`Property\<Person>` by the concrete property type is what ties that Rust type to the `Person` entity.

Never implement `Property\<T>` for a type `T` directly. Instead, use one of the provided macros:

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

### Property Initialization

Every property has one of three initialization behaviors:

| Kind       | How to define it                       | How new entities get a value                                      |
| ---------- | -------------------------------------- | ----------------------------------------------------------------- |
| Explicit   | Omit `default_const`                   | The value must be supplied to `add_entity` with `with!`            |
| Constant   | Add `default_const = ...`              | The default is used unless a value is supplied with `with!`        |
| Derived    | Use `define_derived_property!`         | The value is computed from other properties and cannot be set      |

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

You can still override a constant property's initial value:

```rust
let person = context.add_entity(with!(Person, InfectionStatus::Infectious))?;
```

### Getting and Setting Properties

Once an entity exists, use `get_property` and `set_property`:

```rust
let age: Age = context.get_property(person);
context.set_property(person, Age(age.0 + 1));
```

`set_property` works only for non-derived properties. Derived properties are
computed from their dependencies and update when those dependencies change.

You can also query by property values with `with!`:

```rust
let infectious = context.query_result_iterator(with!(
    Person,
    InfectionStatus::Infectious
));
```

For performance-sensitive queries, see the chapter on
[Indexing](indexing.md).

## Custom Display and Option Properties

ixa uses each property's display implementation in places where a property value
is rendered as text, including reports and diagnostics. By default,
`impl_property!` uses the property's `Debug` representation.

> [!INFO] The `Display` and `Debug` traits in Rust
>
> Rust types implement the `Display` and `Debug` traits to provide a textual representation of the type. `Debug` can be
> automatically `derive`d, shows a value in a developer-focused way, and is typically used for things like error
> messages or logs. `Display` shows a value in a user-facing way. You write it yourself to produce clean, readable
> output.
> 
> In the context of `ixa` properties, a property's "display function" is the function that is used to render a property
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

Since the `display_impl` argument is attached to the `impl Property<Entity> for ConcretePropertyType`, a single
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
standard derives ixa needs, and then calls `impl_property!` for you. The
standard derives are `Debug`, `PartialEq`, `Eq`, `Hash`, `Clone`, `Copy`, and
`serde::Serialize`. The `serde::Serialize` trait is not strictly required, but
it is derived for convenience and compatibility with the reporting system.

Use `impl_property!` when the type already exists or when the type declaration
needs syntax that `define_property!` does not support. The common reasons are:

- You require a different set of derives than `define_property!` generates.
- You need an unsupported type form, such as a type with attributes, generic
  parameters, or more complex Rust syntax.
- You want to use the same Rust type as a property for multiple different
  entity types.

When you use `impl_property!`, you are responsible for making sure the type
implements the traits ixa requires: `Copy`, `Clone`, `Debug`, `PartialEq`,
`Eq`, and `Hash`.

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
    Eq,
    Hash,
    serde::Serialize,
    serde::Deserialize,
)]
pub struct Age(pub u8);

impl_property!(Age, Person);
```

### Example: You need an unsupported type form

`define_property!` intentionally supports a small set of common type
declarations. If the declaration needs field attributes, variant attributes,
generics, non-public fields, or other syntax outside those forms, write the type
directly.

For example, `define_property!` cannot attach a field-level serde attribute:

```rust
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, serde::Serialize)]
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
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
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

The dependency list tells ixa which stored properties affect the derived value.
When one of those dependencies changes, ixa can update indexes and property
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

## Floating Point Types and Implementing `Eq` and `Hash`

Properties participate in equality and hashing, especially when they are
indexed or queried. Plain `f64` and `f32` fields do not implement `Eq` and
`Hash`, so the default derives are not enough for properties that contain
floating-point values.

> [!INFO] Implementing `PartialEq` and `Eq` in Rust
>
> Properties need to implement `Eq`. In practice, this actually means implementing `PartialEq`. In fact, the `Eq` trait
> is just a marker triat—it has no methods! The `Eq` trait is a guarantee by the author that the implementation of
> `PartialEq` [is reflexive](https://doc.rust-lang.org/std/cmp/trait.Eq.html). Rust also requires that `PartialEq`
> is symmetric and transitive.

There are three reasonable ways to handle this.

1. Let ixa generate equality and hashing for you.
2. Implement equality and hashing yourself.
3. Use an alternative floating-point type.

### Let ixa generate equality and hashing

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

This is the shortest option when you want to keep the property as an `f64` and
you are comfortable with ixa's generated equality and hashing behavior. It is a
good fit for simple measured quantities where model code still wants direct
access to a floating-point value.

You can also use only `impl_eq_hash = Eq` or only `impl_eq_hash = Hash` when
one trait can still be derived but the other needs ixa's generated
implementation. Floating-point properties usually need `both`.

The generated implementations are reasonably efficient, but they are not optimal.
If performance is absolutely critical, use either of the other options.

### Implement equality and hashing yourself

Pass `impl_eq_hash = neither` when you want `define_property!` to create the
type but you want to provide `PartialEq` / `Eq` and `Hash` yourself:

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
that gives floating-point values a total ordering and implements the traits ixa
needs:

```rust
use ordered_float::NotNan;

// A type alias is always a good idea here. It allows you to swap out the underlying 
// type without having to change the rest of your code.
pub type Float = NotNan<f64>;

define_property!(
    struct Weight(Float),
    Person,
    default_const = Weight(Float::new(0.0).unwrap())
);
```

With these types, `define_property!` can just `derive` the `PartialEq`, `Eq`, and `Hash` traits it needs, and
performance of these derived implementations is usually optimal. This option is especially attractive when you want to
restrict your type only to the real numbers, or only to the extended real numbers (infinities but not NaNs), giving you
a numeric type with exactly the mathematical semantics you want.

The trade-off is that model code works with the wrapper type instead of a bare `f64`, and while these libraries do what
they can to alleviate friction, having to convert to and from primitive `f64` values is often unavoidable. This is
really the only downside. The good news is, this conversion is usually only cosmetic: the compiler usually optimizes it
away.

## Canonical Values

Sometimes the value you want model code to use is not the value you want ixa to store in indexes. The `canonical_value`,
`make_canonical`, and `make_uncanonical` options let you define a standard representation for internal indexing and
querying that is different from the external value you want to expose to model code:

```rust
define_entity!(WeatherStation);

#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
pub struct DegreesFahrenheit(pub i16);

#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
pub struct DegreesCelsius(pub i16);

impl_property!(
    DegreesFahrenheit,
    WeatherStation,
    canonical_value = DegreesCelsius,
    make_canonical = |value: DegreesFahrenheit| {
        DegreesCelsius(((value.0 - 32) * 5) / 9)
    },
    make_uncanonical = |value: DegreesCelsius| {
        DegreesFahrenheit((value.0 * 9) / 5 + 32)
    },
    display_impl = |value: &DegreesFahrenheit| format!("{} F", value.0)
);
```

The canonical value must satisfy the same equality and hashing requirements as
other property values because it is used directly by indexes.

> [!INFO] Canonical Values and Multi-Properties
>
> Multi-properties use the canonical value mechanism internally so that two tuples
> with the same component properties but having a different component ordering can
> share the same index. A multi-property's canonical value is the tuple of properties
> in lexicographic order. This is all transparent to model code.

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

define_multi_property!((Age, InfectionStatus), Person);

context.index_property::<Person, AgeInfectionStatus>();
```

Use the underlying property names in `define_multi_property!`, not type aliases. For a deeper discussion of when to
create multi-property indexes, see [Indexing](indexing.md). Because the components of a multi-property are already
required to be properties, multi-properties usually "just work".

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
Rust floating-point types do not implement those traits. Choose one of the
floating-point strategies above:

- Add `impl_eq_hash = both` and let ixa generate equality and hashing.
- Add `impl_eq_hash = neither` and manually implement `PartialEq`, `Eq`, and
  `Hash`.
- Wrap the value in a type such as `ordered_float::NotNan\<f64>` or a type from
  `decorum`.

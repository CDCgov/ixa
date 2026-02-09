# Ixa Entity & Property API Reference

This document describes the entity/property system in ixa. It covers both the
**new-style** ZST marker API (v2.0.0) and the **legacy** wrapper-type API.

All examples assume `use ixa::prelude::*;`.

---

## Quick Overview

The entity system has three layers:

1. **Properties** -- marker types that name a value (e.g. `Age` names a `u8`)
2. **Entities** -- zero-sized types that group properties (e.g. `Person` has
   `Age` and `InfectionStatus`)
3. **Context** -- runtime storage. Create entities, get/set property values,
   query

---

## 1. Defining Properties

### Primitive property (new-style)

```rust
define_property!(Age, u8);
define_property!(Weight, f64);
```

This creates a zero-sized struct `Age` implementing `IsProperty<Value = u8>`.
The value type (`u8`) is separate from the marker type (`Age`).

### Enum property (new-style)

```rust
define_property!(
    enum InfectionStatus {
        Susceptible,
        Infected,
        Recovered,
    }
);
```

This creates the enum. The property marker type for it is
`Property<InfectionStatus>`, which automatically implements
`IsProperty<Value = InfectionStatus>` via a blanket impl.

### Legacy wrapper property

The old form ties a property to an entity at definition time and wraps the value
inside the type itself:

```rust
define_property!(struct Age(u8), Person, default_const = Age(0));

define_property!(
    enum InfectionStatus { Susceptible, Infected, Recovered },
    Person,
    default_const = InfectionStatus::Susceptible
);
```

Legacy properties have `Value = Self` (the wrapper IS the value). They still
work and are required for queries and derived properties.

---

## 2. Defining Entities

### Simple entity (no properties)

```rust
define_entity!(Person);
```

Generates `pub struct Person;`, the `Entity` impl, and
`type PersonId = EntityId<Person>`.

### Entity with properties (new-style)

```rust
define_property!(Age, u8);
define_property!(Weight, f64);
define_property!(
    enum InfectionStatus { S, I, R }
);

define_entity!(struct Person {
    Age,                                          // required (no default)
    Weight = 0.0,                                 // optional, default 0.0
    Property<InfectionStatus> = InfectionStatus::S, // enum, default S
});
```

This generates:

- `struct Person;` with `Entity` impl and `PersonId` alias
- `PropertyDef<Person>` impls for each property
- A `PersonBuilder` struct with fluent setter methods
- `Person::build()` returns a `PersonBuilder`

Property syntax inside `define_entity!`:

| Syntax                      | Meaning                                    |
| --------------------------- | ------------------------------------------ |
| `Age,`                      | Required ZST property, no default          |
| `Age = 0,`                  | Optional ZST property with default         |
| `Property<Foo>,`            | Required enum/struct property              |
| `Property<Foo> = Foo::Bar,` | Optional enum/struct property with default |

### Implement Entity for an existing struct

If the struct already exists, use `impl_entity!` instead:

```rust
#[derive(Default, Debug, PartialEq, Eq, Clone, Copy)]
pub struct Vehicle;

impl_entity!(struct Vehicle {
    Speed,
    Property<Fuel> = Fuel::Gas,
});
```

Or without properties:

```rust
impl_entity!(Vehicle);
```

### Legacy entity definition

Legacy entities use `define_entity!` without properties:

```rust
define_entity!(Person);
define_property!(struct Age(u8), Person, default_const = Age(0));
```

---

## 3. Creating Entities

### With builder (new-style entities)

```rust
let person = context.add_entity(
    Person::build().age(30_u8).weight(150.0)
).unwrap();
```

Builder methods are named after the property in `snake_case`. Unset optional
properties use their default. Required properties that aren't set will cause a
runtime error.

### With entity marker (no properties needed)

```rust
let person = context.add_entity(Person).unwrap();
```

### With property tuple (legacy)

```rust
let person = context.add_entity((Age(30), InfectionStatus::Susceptible)).unwrap();
```

### With `all!` macro (legacy, also usable for queries)

```rust
let person = context.add_entity(all!(Person, Age(30), InfectionStatus::S)).unwrap();
```

---

## 4. Getting Property Values

### Turbofish syntax

Works for both new-style and legacy properties:

```rust
// New-style ZST property: returns the Value type (e.g. u8)
let age: u8 = context.get_property::<_, Age>(person);

// New-style enum property: use Property<T> as the type parameter
let status: InfectionStatus = context.get_property::<_, Property<InfectionStatus>>(person);

// Legacy wrapper property: returns the wrapper type (e.g. Age(30))
let age: Age = context.get_property::<_, Age>(person);
```

### Marker argument syntax (new-style only)

Pass the property marker as an argument to avoid turbofish:

```rust
let age: u8 = context.get_property_value(person, Age);
let status = context.get_property_value(person, Property::<InfectionStatus>::new());
```

---

## 5. Setting Property Values

Setting a property always emits a `PropertyChangeEvent`.

### Turbofish syntax

```rust
// New-style ZST
context.set_property::<_, Age>(person, 31_u8);

// New-style enum
context.set_property::<_, Property<InfectionStatus>>(person, InfectionStatus::I);

// Legacy wrapper
context.set_property::<_, Age>(person, Age(31));
```

### Marker argument syntax (new-style only)

```rust
context.set_property_value(person, Age, 31_u8);
context.set_property_value(person, Property::<InfectionStatus>::new(), InfectionStatus::I);
```

---

## 6. Events

### Entity creation

```rust
context.subscribe_to_event::<EntityCreatedEvent<Person>>(|context, event| {
    let new_id: PersonId = event.entity_id;
    // ...
});
```

### Property changes

```rust
use ixa::entity::events::PropertyChangeEvent;

// New-style enum property
type StatusEvent = PropertyChangeEvent<Person, Property<InfectionStatus>>;

// New-style ZST property
type AgeEvent = PropertyChangeEvent<Person, Age>;

// Legacy wrapper property
type AgeEvent = PropertyChangeEvent<Person, Age>;

context.subscribe_to_event::<StatusEvent>(|context, event| {
    let id: PersonId = event.entity_id;
    let old: InfectionStatus = event.previous;  // P::Value
    let new: InfectionStatus = event.current;    // P::Value
    // ...
});
```

`PropertyChangeEvent` fields:

| Field       | Type          | Description                 |
| ----------- | ------------- | --------------------------- |
| `entity_id` | `EntityId<E>` | The entity that changed     |
| `current`   | `P::Value`    | New value after the change  |
| `previous`  | `P::Value`    | Old value before the change |

---

## 7. Queries (Legacy Properties Only)

Queries currently work with legacy wrapper-style properties (where
`Value = Self`). New-style ZST properties cannot be used in tuple queries yet.

### Count entities matching a query

```rust
let count = context.query_entity_count((Age(30), InfectionStatus::I));
```

### Iterate over results

```rust
let iter = context.query_result_iterator((Age(30),));
for person_id in iter {
    // ...
}
```

### Access results as a set

```rust
context.with_query_results((Age(30), InfectionStatus::I), &mut |entity_set| {
    println!("Found {} matches", entity_set.len());
});
```

### Sample a random entity

```rust
define_rng!(MyRng);

let person = context.sample_entity(MyRng, (InfectionStatus::S,));
```

### Check if an entity matches

```rust
let matches: bool = context.match_entity(person, (Age(30),));
```

### `all!` macro for entity-scoped queries

The `all!` macro wraps a property tuple with an entity type for disambiguation:

```rust
let count = context.query_entity_count(all!(Person, Age(30)));
let person = context.sample_entity(MyRng, all!(Person, InfectionStatus::S));
```

This is useful when the entity type can't be inferred from context.

### Indexing for performance

```rust
context.index_property::<Person, Age>();
```

Indexing makes queries on that property O(1) instead of O(n). The index is built
lazily on first query after calling `index_property`.

---

## 8. Derived Properties (Legacy Only)

Derived properties are computed from other properties and update automatically:

```rust
define_derived_property!(
    enum AgeGroup { Child, Adult, Senior },
    Person,
    [Age],
    |age| {
        if age.0 < 18 { AgeGroup::Child }
        else if age.0 < 65 { AgeGroup::Adult }
        else { AgeGroup::Senior }
    }
);
```

Derived properties:

- Are read-only (cannot be set)
- Automatically recompute when dependencies change
- Emit `PropertyChangeEvent` when their value changes
- Can be queried and indexed like any other property

---

## 9. Multi-Properties (Legacy Only)

Index and query multiple properties jointly:

```rust
define_multi_property!((Age, County, Height), Person);

// Queries on these properties can use the joint index
context.index_property::<Person, (Age, County, Height)>();
context.with_query_results((Age(30), County(5), Height(170)), &mut |results| {
    // ...
});
```

---

## 10. Macro Reference

### Property macros

| Macro                                                 | Purpose                                            |
| ----------------------------------------------------- | -------------------------------------------------- |
| `define_property!(Name, ValueType)`                   | New-style ZST property marker                      |
| `define_property!(enum Name { ... })`                 | New-style enum property                            |
| `define_property!(struct Name(Type), Entity, ...)`    | Legacy wrapper property (struct)                   |
| `define_property!(enum Name { ... }, Entity, ...)`    | Legacy wrapper property (enum)                     |
| `impl_property!(Name, Entity, ...)`                   | Implement `PropertyDef` for existing type (legacy) |
| `impl_property_for_entity!(Name, Entity)`             | Associate ZST property with entity                 |
| `impl_property_for_entity!(Property<T>, Entity, ...)` | Associate enum property with entity                |

### Entity macros

| Macro                                 | Purpose                                              |
| ------------------------------------- | ---------------------------------------------------- |
| `define_entity!(Name)`                | Define entity struct + Entity impl                   |
| `define_entity!(struct Name { ... })` | Define entity with property declarations + builder   |
| `impl_entity!(Name)`                  | Implement Entity for existing struct                 |
| `impl_entity!(struct Name { ... })`   | Implement Entity for existing struct with properties |

### Other macros

| Macro                                           | Purpose                                  |
| ----------------------------------------------- | ---------------------------------------- |
| `all!(Entity, prop_values...)`                  | Create entity-scoped query/property list |
| `define_derived_property!(...)`                 | Computed property from other properties  |
| `define_multi_property!((P1, P2, ...), Entity)` | Joint property index                     |

---

## 11. Type Cheat Sheet

### New-style property types

```rs
define_property!(Age, u8)
  Age             : IsProperty<Value = u8>     -- ZST marker
  Age             : PropertyDef<Person>         -- after define_entity!
  P::Value = u8                                 -- what get/set operate on

define_property!(enum Status { S, I, R })
  Property<Status> : IsProperty<Value = Status> -- blanket impl
  Property<Status> : PropertyDef<Person>        -- after define_entity!
  P::Value = Status                             -- what get/set operate on
```

### Legacy property types

```rs
define_property!(struct Age(u8), Person)
  Age             : PropertyDef<Person>
  P::Value = Age                                -- wrapper IS the value
```

### Key traits

| Trait               | Purpose                                                             |
| ------------------- | ------------------------------------------------------------------- |
| `Entity`            | Marker for entity types. Impl via `define_entity!` / `impl_entity!` |
| `IsProperty`        | Entity-independent property marker. Has `type Value`                |
| `PropertyDef<E>`    | Ties a property to an entity. Has `type Value`, storage, indexing   |
| `PropertyList<E>`   | For entity creation. Tuples, builders, entity markers               |
| `Query<E>`          | For querying. Tuples of legacy properties, `all!` macro results     |
| `PropertySetter<E>` | For `(Marker, Value)` tuples                                        |

### IDs

| Type          | Description                                        |
| ------------- | -------------------------------------------------- |
| `EntityId<E>` | Typed ID for an entity instance                    |
| `PersonId`    | Type alias for `EntityId<Person>` (auto-generated) |

---

## 12. Full Example (New-Style)

```rust
use ixa::prelude::*;
use ixa::entity::events::PropertyChangeEvent;

// 1. Define properties
define_property!(Age, u8);
define_property!(Weight, f64);
define_property!(
    enum InfectionStatus { S, I, R }
);

// 2. Define entity with properties
define_entity!(struct Person {
    Age,
    Weight = 0.0,
    Property<InfectionStatus> = InfectionStatus::S,
});

// 3. Event type alias
type InfectionEvent = PropertyChangeEvent<Person, Property<InfectionStatus>>;

fn init(context: &mut Context) {
    // Subscribe to infection changes
    context.subscribe_to_event::<InfectionEvent>(|context, event| {
        if event.current == InfectionStatus::I {
            // schedule recovery...
        }
    });

    // Create entities
    let alice = context
        .add_entity(Person::build().age(30_u8).weight(130.0))
        .unwrap();

    let bob = context
        .add_entity(Person::build().age(45_u8))  // weight defaults to 0.0
        .unwrap();

    // Read properties
    let age: u8 = context.get_property_value(alice, Age);
    assert_eq!(age, 30);

    // Update properties (emits InfectionEvent)
    context.set_property_value(alice, Property::<InfectionStatus>::new(), InfectionStatus::I);

    // Entity count
    assert_eq!(context.get_entity_count::<Person>(), 2);
}
```

## 13. Full Example (Legacy Style)

```rust
use ixa::prelude::*;
use ixa::entity::events::PropertyChangeEvent;

// 1. Define entity
define_entity!(Person);

// 2. Define properties tied to entity
define_property!(struct Age(u8), Person, default_const = Age(0));
define_property!(
    enum InfectionStatus { S, I, R },
    Person,
    default_const = InfectionStatus::S
);

// 3. Event type alias
type InfectionEvent = PropertyChangeEvent<Person, InfectionStatus>;

fn init(context: &mut Context) {
    // Subscribe to infection changes
    context.subscribe_to_event::<InfectionEvent>(|context, event| {
        if event.current == InfectionStatus::I {
            // schedule recovery...
        }
    });

    // Create entities with tuple
    let alice = context.add_entity((Age(30),)).unwrap();

    // Read properties (returns wrapper type)
    let age: Age = context.get_property::<_, Age>(alice);
    assert_eq!(age, Age(30));

    // Update properties
    context.set_property::<_, InfectionStatus>(alice, InfectionStatus::I);

    // Query
    let infected_count = context.query_entity_count((InfectionStatus::I,));
    assert_eq!(infected_count, 1);

    // Sample from query
    define_rng!(MyRng);
    context.init_random(42);
    let random_susceptible = context.sample_entity(MyRng, (InfectionStatus::S,));
}
```

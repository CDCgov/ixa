# Example: Entities and Properties

This example demonstrates how to:

- Add `Person` entities to a simulation
- Define `Person` properties and initialize them
- Load a population from a CSV file into entity properties
- Subscribe to entity creation and property change events

## People (as entities)

`Person` is an `Entity` type, and `PersonId = EntityId<Person>`. Both are
available via the prelude:

```rust
use ixa::prelude::*;

let mut context = Context::new();
let person: PersonId = context.add_entity(()).unwrap();
println!("Person {} was created", person.0);
```

## Properties

In the entities API, the _property type is the value type_. You define a type
and then make it a `Property<Person>`.

For example, this model uses an `Age` newtype and a `RiskCategory` enum:

```rust
use ixa::prelude::*;
use serde::{Deserialize, Serialize};

define_property!(struct Age(pub u8), Person);

#[derive(Serialize, Deserialize, Copy, Clone, PartialEq, Eq, Debug, Hash)]
pub enum RiskCategory { High, Low }
ixa::impl_property!(RiskCategory, Person);
```

To get and set properties:

```rust
let person: PersonId = context.add_entity((Age(69), RiskCategory::Low)).unwrap();
let Age(age) = context.get_property::<Person, Age>(person);
context.set_property(person, Age(age + 1));
```

### Default values

If a property should have a default for every new entity, use `default_const`.
For example, `sir.rs` defines `DiseaseStatus` with a default:

```rust
define_property!(
    enum DiseaseStatus { S, I, R },
    Person,
    default_const = DiseaseStatus::S
);
```

### Loading from CSV

This example loads CSV rows and initializes entities by passing property values
directly to `add_entity`. Initial values set via `add_entity` do **not** emit
`PropertyChangeEvent`s.

## Observing events

Models can subscribe to:

- `EntityCreatedEvent<Person>` to observe new entities
- `PropertyChangeEvent<Person, P>` to observe updates to a specific property `P`

For example:

```rust
context.subscribe_to_event(|_context, event: EntityCreatedEvent<Person>| {
    println!("Created {:?}", event.entity_id);
});

context.subscribe_to_event(|_context, event: PropertyChangeEvent<Person, DiseaseStatus>| {
    println!("{:?} changed from {:?} to {:?}", event.entity_id, event.previous, event.current);
});
```

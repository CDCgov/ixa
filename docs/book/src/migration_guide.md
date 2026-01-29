# Migration to ixa 2.0

## Properties in the new Entity system

In the new Entity system, properties work a little differently.

### The Old Way to Define a Property

Previously a property consisted of two distinct types: the type of the
property's _value_, and the type that identifies the property itself.

```rust
// The property _value_ type, any regular Rust datatype that implements
// `Copy` and a few other traits, which we define like any other Rust type.
#[derive(Debug, Hash, Eq, PartialEq, Clone, Copy, Serialize, Deserialize)]
pub enum InfectionStatusValue {
    Susceptible,
    Infected,
    Recovered,
}

// The type identifying the property itself. A macro defines this as a ZST and
// generates the code connecting it to the property value type above.
define_person_property_with_default!(
    InfectionStatus,      // Property Name
    InfectionStatusValue, // Type of the Property Values
    InfectionStatusValue::Susceptible // Default value used when a person is added to the simulation
);
```

The only entity in the old system is a person, so there's no need to specify
that this is a property for the `Person` entity.

### The New Way to Define a Property

In the new system, we combine these two types into a single type:

```rust
// We now have an `Entity` defined somewhere.
define_entity!(Person);
// This macro takes the entire type declaration itself as the first argument.
define_property!(
    enum InfectionStatus {
        Susceptible,
        Infected,
        Recovered,
    },
    Person,
    default_const = InfectionStatus::Susceptible
);
```

If you want to use an existing type, or if you want to make the same type a
property of more than one `Entity`, you can use the `impl_property!` variant:

```rust
// The downside is, we have to make sure the property type implements all the
// traits a property needs.
#[derive(Copy, Clone, Debug, PartialEq, Serialize)]
pub enum InfectionStatus {
    Susceptible,
    Infected,
    Recovered,
}

// Implements `Property<Person>` for an existing type.
impl_property!(
    InfectionStatus,
    Person,
    default_const = InfectionStatus::Susceptible
);
```

(In fact, the _only_ thing `define_property!` does is tack on the `derive`
traits and `pub` visibility to the type definition and then call
`impl_property!` as above.)

The crucial thing to understand is that the value type _is_ the property type.
The `impl Property<Person> for InfectionStatus`, which the macros give you, is
the thing that ties the `InfectionStatus` type to the `Person` entity.

For details about defining properties, see the `property_impl` module-level docs
and API docs for the macros `define_property!`, `impl_property!`,
`define_derived_property!`, `impl_derived_property!`, and
`define_multi_property!`. The API docs give multiple examples.

### Summary

| Concept                     | Old System                                                                                                                     | New System                                                                                                           | Notes / Implications                                                                |
| --------------------------- | ------------------------------------------------------------------------------------------------------------------------------ | -------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------- |
| **Property Type Structure** | Two separate types: (1) value type, and (2) property-identifier ZST.                                                           | A single type represents both the value and identifies the property.                                                 | Simplified to a single type                                                         |
| **Defining Properties**     | Define a normal Rust type (or use an existing primitive, e.g. `u8`), then use a macro to define the identifying property type. | Define a normal Rust type, them use `impl_property` to declare it a `Property<E>` for a particular `Entity` `E`.     |                                                                                     |
| **Entity Association**      | Implicit—only one entity ("person")                                                                                            | Every property must explicitly specify the `Entity` it belongs to (e.g., `Person`); Entities are defined separately. |                                                                                     |
| **Default Values**          | Provided in the macro creating the property-identifier type.                                                                   | Same but with updated syntax; default values are per `Property<E>` implementation.                                   |                                                                                     |
| **Using Existing Types**    | A single _value_ type can be used in multiple properties—including primitive types like `u8`                                   | Only one property per type (per `Entity`); primitive types must be wrapped in a newtype.                             | Both systems require that the existing type implement the required property traits. |
| **Macro Behavior**          | Macros define the property’s ZST and connect it to the value type via trait impls.                                             | Macros define "is a property of entity `E`" relationship via trait impl. No additional synthesized types.            | Both enforce correctness via macro                                                  |

## New Entities API: How Do I...?

We will use `Person` as an example entity, but there is nothing special about
`Person`. We could just as easily define a `School` entity, or an `Animal`
entity.

### Defining a new `Entity`

Use the `ixa::define_entity!` macro:

```rust
define_entity!(Person);
```

This both _declares_ the type `Person` and implements the `Entity` trait for it.
If you want to implement the `Entity` trait for a type you have _already
declared_, use the `impl_entity!` macro instead:

```rust
#[derive(Default, Debug, PartialEq, Eq, Clone, Copy)]
pub struct Person;
impl_entity!(Person);
```

These macros automatically create a type alias of the form
`MyEntityId = EntityId<MyEntity>`. In our case, it defines the type alias
`PersonId = EntityId<Person>`.

### Adding a new entity (e.g. a new person)

Adding a new entity with multiple property values:

```rust
// Assuming the `Person` entity is defined somewhere.
// Add a new entity (a person in this case) to an existing `Context` instance we have access to.
let person_id = context.add_entity((Age(25), InfectionStatus::Infected)).unwrap();
```

Observe:

- The compiler is smart enough to know that we are adding a new `Person` entity
  because we supplied a list of property values that are properties for a
  `Person`.
- The `add_entity` function takes a "property list", which is just a tuple of
  property values. The properties must be distinct, of course, and there must be
  a value for every "required" property, that is, for every (non-derived)
  property that doesn't have a default value.
- A single-property tuple uses the syntax `(Age(25), )`. Notice the awkward
  trailing comma, which lets the compiler know the parentheses are defining a
  tuple rather than functioning as grouping an expression.

Adding a new entity with just one property value:

```rust
// Assuming the `Person` entity is defined somewhere.
// Add a new entity (a person in this case) to an existing `Context` instance we have access to.
let person_id = context.add_entity((Age(25), )).unwrap();
```

Adding a new entity with only default property values:

```rust
// If you specify the `EntityId<E>` return type, the compiler uses it to infer which entity to add.
// This is a good practice and avoids the special "turbo fish" syntax.
let person_id: PersonId = context.add_entity(()).unwrap();

// If we don't specify the `EntityId<E>` type, we have to explicitly tell the compiler *which* entity
// type we are adding, as there is nothing from which to infer the entity type.
let person_id = context.add_entity::<Person, _>(()).unwrap();
```

(These two examples assume there are no required properties, that is, that every
property has a default value.)

### Getting a property value for an entity

```rust
// The compiler knows which property to fetch because of the type of the return value we have specified.
let age: Age = context.get_property(person_id);
```

In the rare situation in where the compiler cannot infer which property to
fetch, you can specify the property explicitly using the turbo fish syntax. We
recommend you always write your code in such a way that you can use the first
version.

```rust
let age = context.get_property::<Person, Age>(person_id);
```

### Setting a property value for an entity

```rust
// The compiler is always able to infer which entity and property using the `EntityId` and type
// of the property value.
context.set_property(person_id, Age(35));
```

### Index a property

```rust
// This method is called with the turbo-fish syntax, because there is no way for the compiler to infer
// the entity and property types.
context.index_property::<Person, Age>();
```

It is not an error to call this method multiple times for the same property. All
calls after the first one will just be ignored.

### Subscribe to events

The only difference is that now we use the
`PropertyChangeEvent\<E: Entity, P: Property<E>>` and
`EntityCreatedEvent\<E: Entity>` types.

```rust
pub fn init(context: &mut Context) {
    context.subscribe_to_event(
        move |context, event: PropertyChangeEvent<Person, InfectionStatus>| {
            handle_infection_status_change(context, event);
        },
    );

    context.subscribe_to_event(move |context, event: PropertyChangeEvent<Person, Alive>| {
        handle_person_removal(context, event);
    });
}
```

## Renamed Methods provided by `ContextPeopleExt`

| Old                                      | New                                             | Description                                                            |
| ---------------------------------------- | ----------------------------------------------- | ---------------------------------------------------------------------- |
| `register_property<T: PersonProperty>()` | `register_person_property<T: PersonProperty>()` |                                                                        |
| `index_property<T: PersonProperty>()`    | `index_person_property<T: PersonProperty>()`    | The new `index_property` method works for `Property<E: Entity>` types. |
| `with_query_results<Q: Query>()`         | `with_query_people_results<Q: Query>()`         | The new `with_query_results` method queries entity storage.            |

## Renamed Macros

| Old                        | New                               | Description                                                                  |
| -------------------------- | --------------------------------- | ---------------------------------------------------------------------------- |
| `define_derived_property!` | `define_derived_person_property!` | The new `define_derived_property!` defines a new `Property<E: Entity>` type. |

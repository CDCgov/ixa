/*!

A "person" is represented by a `PersonId` and has multiple `PersonProperty` values
associated with it. Entities generalize this: An `Entity` is analogous to a table in
a relationship database, and the properties of an entity are analogous to the columns
that exist on the table. A row in the table is addressable with an `EntityId<Entity>`
(implemented as a newtype of a `usize`). In this new paradigm, `Person` is a particular
entity, possibly one of several, and `PersonId` is a type alias for `EntityId<Person>`.

Entity property getters and setters exists on `Context` like this:

```rust,ignore
// The `my_entity_id` value is of type `MyEntityId`, which is a type alias for `EntityId<MyEntity>`.
// (The `MyProperty` type knows which entity it belongs to.)
let my_property_value = context.get_property::<MyProperty>(my_entity_id);
// Turbofish-less version of the same call:
let my_property_value: MyProperty = context.get_property(my_entity_id);

// For setters, the property is inferred from the type of the value we are passing in.
context.set_property(my_entity_id, some_property_value);
// ...but if you want to be super-explicit, you could use a turbofish version:
context.set_property::<MyProperty>(my_entity_id, some_property_value);
```

This implementation of entities relies heavily on the "registry pattern" for efficient
lookup of entities and properties. The idea is that concrete types implementing `Entity` (and
separately, `Property<Entity>`) have a `ctor` that initializes a global (per concrete type)
static variable `index`. Each concrete `Entity` type is thus assigned a unique index ranging from
`0` to `ENTITY_COUNT - 1`. Then instances of container types like `EntityStore` (respectively
`PropertyStore`) use this index to look up the corresponding instances in a vector it owns.

*/

pub mod context_extension;
mod entity;
mod entity_impl;
pub mod entity_store;
pub mod events;
mod index;
pub mod multi_property;
pub mod property;
pub mod property_impl;
pub mod property_list;
pub mod property_store;
pub(crate) mod property_value_store;
pub(crate) mod property_value_store_core;
pub mod query;

// Flatten the module hierarchy.
pub use context_extension::ContextEntitiesExt;
pub use entity::*;
pub use entity_impl::*;
pub(crate) use query::Query;

/// The type used in the indexing infrastructure. This type alias is
/// public, because it is used by any implementor of `Property<E: Entity>`.
pub type HashValueType = u128;

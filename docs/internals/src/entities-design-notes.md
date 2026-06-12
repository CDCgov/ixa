# Entity System Design Notes

This chapter describes the current internal architecture of Ixa's entity system.
It is not intended to duplicate the user-facing entity and property guides in
the main Ixa Book. For user-facing syntax and examples, see the migration guide,
the indexing chapter, and the forthcoming properties chapter in the Ixa Book.

Historical notes are kept here only when they explain the current design or a
still-open design question.

## Entity Model

An `Entity` is a type-level marker for a collection of related properties. In
model code, entities are usually declared with `define_entity!`, and existing
types can implement the trait with `impl_entity!`.

The `define_entity!` macro also creates the conventional entity ID alias. For
example, `define_entity!(Person)` creates `PersonId = EntityId<Person>`.

`EntityId<E>` is intentionally typed by entity. A `PersonId` cannot be passed
where a `SchoolId` is expected, even though both are represented internally as a
row index. The row index itself is opaque outside `ixa`; only Ixa internals can
construct new `EntityId<E>` values.

Entity counts do not live on the entity marker type. They live in the
per-context `EntityStore`, because entity marker types are defined by client
code and should not be able to create IDs or modify population counts directly.

`PopulationIterator<E>` iterates over the valid `EntityId<E>` values for an
entity type. It captures the population size when the iterator is created, so
entities added later are not included in that iterator.

## Registration and Store Ownership

`Context` owns an `EntityStore` directly. The `EntityStore` contains one
`EntityRecord` for each registered entity type. Each record tracks the current
entity count and lazily initializes the entity's `PropertyStore<E>`.

Registration uses macro-generated `ctor`s. At startup, entity and property
metadata is collected into global registries. The metadata is frozen on first
read, and late registration after that point is treated as an internal error.

There are two kinds of IDs in play:

- `TypeId` is used where the question is type identity, such as validating that
  an initialization list contains the required property types.
- Numeric entity and property IDs are used for fast lookup in stores.

Property IDs are scoped to an entity type. It is possible for `Property<E1>` and
`Property<E2>` to have the same numeric property ID when `E1 != E2`. Internal
metadata that needs a stable property key therefore uses `(E::id(), P::id())`.

Each `PropertyStore<E>` contains one type-erased
`PropertyValueStoreCore<E, P>` for each registered property of `E`. This is a
change from older notes that described lazily initializing individual property
stores from a `Vec<OnceCell<Box<dyn Any>>>`. Today, the `PropertyStore<E>`
itself is lazy, but once it exists its property value stores are constructed
from the frozen property metadata.

## Property Model

The Rust value type is also the property type. The trait implementation
`impl Property<Person> for Age` is what makes `Age` a property of `Person`.

All property values satisfy the internal `AnyProperty` bounds:

```rust
Copy + Debug + PartialEq + Eq + Hash + 'static
```

Those bounds matter because property values and canonical values can be used as
index keys, query keys, event payloads, and stored column values.

Every property has one of three initialization kinds:

- `Explicit`: the value must be supplied when the entity is created.
- `Constant`: a constant default is used unless creation supplies a value.
- `Derived`: the value is computed from dependencies and cannot be set directly.

The old "required versus explicit" question is resolved in the implementation:
a non-derived property without `default_const` is explicit, and explicit
properties are required during `add_entity`.

Macros remain the intended way to implement the `Entity` and `Property` traits
correctly. In particular:

- `define_property!` defines a public property type and delegates to
  `impl_property!`.
- `impl_property!` attaches an existing type to an entity.
- `define_derived_property!` records dependency information used to update
  derived-property indexes and events.
- `define_multi_property!` uses canonical values and shared `index_id()`
  machinery to support multi-property indexes.

`define_property!` emits public generated types and public generated fields for
struct properties. When model code needs custom visibility, attributes,
additional derives, or more complex Rust syntax, it should define the type
itself and use `impl_property!`.

### Property IDs and Index IDs

Most properties use their own property ID as their index ID. Multi-properties
are the important exception. Equivalent multi-properties, such as
`(Age, InfectionStatus)` and `(InfectionStatus, Age)`, share a single
underlying index. For that reason `Property::id()` answers "which property is
this?" while `Property::index_id()` answers "which property value store owns the
index this property should use?"

### Canonical Values

`Property::CanonicalValue` is the internal representation used by indexes and
indexed query lookup. For ordinary properties this is usually the property type
itself. For multi-properties, the canonical value is the component tuple in a
stable order, allowing equivalent multi-properties to share one index.

The Ixa Book's properties chapter covers user-facing property syntax, custom
display behavior, `Option<T>` properties, floating-point equality and hashing,
and canonical values in more detail. This internals chapter only depends on the
fact that indexes use canonical values as keys.

## Property Storage

`PropertyValueStoreCore<E, P>` owns the storage and index state for one
property:

- `data: Vec<P>` stores non-derived property values.
- `index: PropertyIndex<E, P>` stores the property's current index, if any.
- value-change counters are stored alongside the property.

Derived properties have no backing value vector. `Context::get_property`
computes them from the current context and entity ID.

Constant-default properties use a storage optimization: trailing default values
do not have to be materialized in `data`. If a constant property has not stored
a value for an entity ID, `get` can return `P::default_const()`.

Explicit properties do not have that fallback. The entity creation path enforces
that every explicit property has a value before the new entity can be created.

## Add Entity Flow

`Context::add_entity` currently returns `Result<EntityId<E>, IxaError>`.

The flow is:

1. Validate that the supplied property list contains distinct property types.
2. Check that all required properties are present.
3. Create a new typed `EntityId<E>`.
4. Write initial property values into the `PropertyStore<E>`.
5. Catch up enabled indexes for newly added entities.
6. Emit `EntityCreatedEvent<E>`.
7. Return the new ID.

Initial property writes during entity creation do not emit property-change
events. Entity creation has its own `EntityCreatedEvent<E>`.

Public entity initialization uses either the entity marker type for all-default
initialization:

```rust
context.add_entity(Person)
```

or the `with!` macro for explicit values:

```rust
context.add_entity(with!(Person, Age(42), InfectionStatus::Susceptible))
```

Public initialization APIs no longer accept naked tuples such as
`(Age(42),)`.

### Open Question: Fallible or Panicking `add_entity`

The old design question about `add_entity` is still live. The current API
returns `Result`, but the tradeoff remains:

- Returning `Result` makes sense for cases where entity creation might be
  driven by external input, a debugging interface, or a web/API layer.
- Panicking can be more ergonomic for ordinary model code, where an invalid
  initialization list is a programmer error and recovery is unlikely.
- A possible future API split would be `add_entity` for the common panicking
  path and `try_add_entity` for fallible callers.

For now, the implementation remains fallible.

## Set Property and Derived Dependents

`Context::set_property` can only set non-derived properties. Derived properties
are recomputed from their dependencies.

The current algorithm is:

1. Snapshot previous values for the property being set and any dependent
   derived properties that need change processing.
2. Write the new non-derived property value.
3. Emit the partial change events. Each partial event recomputes the current
   value, updates value-change counters when the value actually changed,
   updates indexes by removing the previous value and inserting the current
   value, and emits a `PropertyChangeEvent`.

`set_property` intentionally emits property-change events even when
`current == previous`. This treats the event as a report of a write transaction,
not just a report that the instantaneous state changed. Code that only cares
about real value changes can compare `event.current` and `event.previous`.

Value-change counters are stricter: they update only when
`current != previous`.

The old allocation concern around partial property-change events has been
partly addressed. Partial events use `SmallBox`, and the dependent-event list
inside `set_property` uses `SmallVec`.

### Why Index Catch-Up Uses Narrow `unsafe`

Index storage used to rely on interior mutability. The current design removed
`RefCell` from `PropertyIndex`, so index reads return plain references and
index writes require mutable references. This makes query and iterator
reference types simpler and avoids carrying runtime borrow guards through
`EntitySet` and `EntitySetIterator`.

The cost is a narrow use of `unsafe` in the index catch-up paths used by
`ContextEntitiesExt::add_entity` and `ContextEntitiesExt::index_property`.

The core issue is that index catch-up mutates a `PropertyStore<E>`, while
indexing derived properties may need a shared `&Context` to compute
`P::compute_derived(context, entity_id)`. Rust can express partial borrows of
different fields in local code, but this pattern crosses method boundaries
through `Context`. The implementation therefore uses a raw context pointer to
create a shared context reference while mutating the relevant property store.

The intent is not arbitrary aliasing. The mutable access is limited to index
internals, and the shared context reference is used for read-only property
access needed to compute derived values.

## Query Model

Public query APIs use either:

- `with!(Entity, prop1, prop2, ...)` for property filters, or
- the entity marker itself, such as `Person`, for a whole-population query.

The unit type `()` still exists internally as an empty query, but it is not the
preferred public API. Query tuples are wrapped in `EntityPropertyTuple<E, T>` so
the query carries the entity type explicitly.

The main public query methods are:

- `query(query) -> EntitySet<E>`
- `query_result_iterator(query) -> EntitySetIterator<E>`
- `with_query_results(query, callback)` for scoped `EntitySet` access
- `query_entity_count(query)`
- `sample_entity`, `count_and_sample_entity`, and `sample_entities`
- `get_entity_iterator::<E>()` for whole-population iteration

Use `query_result_iterator` for ordinary iteration. Use `query_entity_count`
for counts. `with_query_results` exists for code that needs direct access to an
`EntitySet`, especially when an indexed query can be represented by borrowing
an indexed source without constructing an intermediate vector.

The scoped callback matters because an `EntitySet` may hold immutable
references into `Context`, such as a reference to an index bucket. While that
set is live, the context cannot be mutably borrowed. `with_query_results`
contains that borrow inside the callback.

The details of `EntitySet` and `EntitySetIterator` live in the
[EntitySet](entity_set.md) chapter.

## Indexing and Multi-Properties

Indexes are per-context. Enabling an index affects only that `Context`.

There are two index modes:

- `context.index_property::<E, P>()` enables a full index.
- `context.index_property_counts::<E, P>()` enables a value-count index unless
  a full index already exists.

Full indexes support both query result sets and counts. Value-count indexes
support counts but not entity-set lookup.

Enabling an index catches it up to the current population. After that, indexes
are maintained during entity creation and property changes.

`is_property_indexed` exists only under `#[cfg(test)]`. It is useful for tests,
but client code does not need to ask whether an index is already enabled.
Calling an indexing method more than once is not an error.

### Multi-Properties

A multi-property is a derived tuple property created with
`define_multi_property!`:

```rust
define_multi_property!((Age, InfectionStatus), Person);
```

The main use is joint indexing: a query over `Age` and `InfectionStatus` can
use the shared multi-property index instead of intersecting separate component
sources.

Equivalent multi-properties with reordered components share one `index_id`.
For example, `(Age, InfectionStatus)` and `(InfectionStatus, Age)` point to the
same underlying index. Component values are canonicalized into a stable order
before indexed lookup.

Events remain type-specific through the normal `PropertyChangeEvent<E, P>`
machinery. A multi-property is still a property type, even when its index is
shared with an equivalent ordering.

Multi-properties are not indexed automatically. The current behavior is
explicit indexing:

```rust
context.index_property::<Person, AgeInfectionStatus>();
```

The old auto-indexing question remains worth preserving. Multi-properties have
uses besides joint indexing: they can serve as event types, derived tuple
properties, and value-combination counting keys. Automatic indexing would impose
memory and maintenance costs even when a multi-property is being used for one
of those non-index purposes.

## Events

The entity subsystem emits two core event types:

- `EntityCreatedEvent<E>` after successful entity creation.
- `PropertyChangeEvent<E, P>` after property writes and derived dependent
  updates.

Property-change events are part of the `set_property` flow. A write to one
non-derived property can emit events for that property and for derived
properties whose values may have changed because of the write.

# ADR-0001: Adopt a typed entity and property architecture

| Field | Value |
| --- | --- |
| Decision date | 2026-02-09 (adoption on `main`) |
| Recorded | 2026-07-29 |
| Status | Accepted |
| GitHub issues | [#389](https://github.com/CDCgov/ixa/issues/389) |
| Feature branches | `RobertJacobsonCDC_entities` |

## Summary

Ixa replaced its person-specific property subsystem with a general entity
architecture. Entity marker types identify collections of properties, opaque
`EntityId&lt;E&gt;` values identify rows of a particular entity type, and a property's
Rust value type implements `Property&lt;E&gt;` directly. A context owns the entity
counts and property storage for all registered entity types.

Property reads were also made non-mutating. Non-derived properties either had a
constant default or required an explicit value at entity creation, while
derived properties were computed from other state. Removing dynamic
initialization on first read allowed queries to retain immutable views of the
context without a hidden initialization path requiring mutation.

## Context

The earlier subsystem treated a person as Ixa's only entity. Its property API
used two types for one conceptual property: a zero-sized tag carried the
property behavior, and an associated value type carried the data. This produced
person-specific APIs and names, made property values less useful for type
inference, and required callers to pass property tags in places where the value
type could otherwise identify the property.

The replacement needed to support multiple kinds of modeled objects, such as
people, schools, or settings, without giving up useful guarantees of the
person-specific system. In particular:

- IDs needed to remain opaque so client code could not manufacture arbitrary
  rows.
- The compiler needed to distinguish IDs and properties belonging to different
  entity types.
- Ixa needed to discover client-defined entity and property types and allocate
  per-context storage for them.
- Required initialization values had to be validated when an entity was
  created.
- Query execution needed to hold immutable views of stored values and indexes.

The old property initialization model complicated the last requirement. A
property could be initialized dynamically when first read. Consequently, an
apparently immutable read could need to create storage or write a value.
Derived properties inherited the same problem through their dependencies.
Query implementations that retained immutable context borrows could therefore
encounter conflicting mutation or runtime borrow failures.

## Decision

Ixa adopted the following entity and property model:

1. An `Entity` implementation is a type-level marker for a collection of
   related properties. `define_entity!` and `impl_entity!` provide the supported
   implementations.
2. `EntityId&lt;E&gt;` is an opaque, entity-typed row identifier. Its numeric value is
   constructed only inside Ixa, and macro-generated aliases such as
   `PersonId = EntityId&lt;Person&gt;` preserve convenient domain names. An ID for one
   entity type cannot be used where another entity type's ID is required.
3. The Rust value type is also the property type. Implementing `Property&lt;E&gt;`
   associates that type with entity `E`; `define_property!`,
   `impl_property!`, and the derived-property macros generate the required
   implementations and registration code. This replaced the separate
   property-tag and property-value types.
4. Each `Context` owns an `EntityStore`. It holds a record for every registered
   entity type, including that entity's population count and lazily initialized
   typed property store. Numeric registry IDs provide fast store lookup, while
   type erasure and downcasting are confined to the heterogeneous registry
   boundaries.
5. Macro-generated startup registration records entity and property metadata.
   Property IDs are scoped to their entity type, and initialization lists are
   checked for distinct properties and all required property types before a new
   ID is committed.
6. A property has one of three initialization modes:
   - **Explicit:** a value must be supplied when the entity is created.
   - **Constant:** a compile-time constant is used when creation does not supply
     a value.
   - **Derived:** the value is computed from dependencies and cannot be set
     directly.

Dynamic initialization based on the state observed at first read was removed.
Reading a constant property could return its default without writing it, and
derived-property computation used immutable context access. Non-constant
initial state therefore had to be supplied explicitly at entity creation.

This decision established the typed entity/property foundation. The detailed
representation and ownership of query results, and later changes to index
ownership and maintenance, were separate decisions.

## Rationale

Parameterizing IDs and properties by entity type moved important correctness
checks into Rust's type system. It prevented accidental mixing of, for example,
person and school IDs, and ensured APIs could infer the relevant entity from an
ID or property value in common cases.

Making the value type itself the property type removed an artificial
tag/value split. It made signatures such as `get_property` and `set_property`
express the modeled value directly, while still allowing the same Rust type to
implement `Property&lt;E&gt;` for more than one entity when needed.

A dedicated `EntityStore` gave the entity subsystem its own ownership,
initialization, and access semantics instead of coupling those choices to the
generic data-plugin registry. Per-context ownership kept population and
property state local to a simulation context; global registration was limited
to type metadata and numeric lookup assignments.

Finally, eliminating read-time dynamic initialization gave property access a
clear invariant: reading did not mutate the context. That invariant was simpler
than maintaining separate mutable and immutable compute APIs or relying on a
two-path implementation whose correctness depended on callers running a
mutation phase before every query.

## Consequences

The architecture generalized Ixa's population model beyond people while
preserving compact numeric row storage. Entity IDs, property associations,
events, stores, and query APIs could all carry the entity type statically.
Client code gained simpler property types and stronger type inference, and
immutable query iteration no longer depended on hidden writes during property
access.

The change was a substantial migration. Existing person-property declarations,
method names, events, examples, and models had to move to the entity-aware API.
Property value types needed the traits required by property storage and
events, and distinct conceptual properties generally needed distinct Rust
newtypes rather than sharing a primitive value type within one entity.

Automatic type discovery and fast numeric lookup introduced macro-generated
registration, startup constructors, globally assigned metadata IDs, and
type-erased registry slots. Correct registration depended on using the supplied
macros, and heterogeneous storage required checked downcasts at its internal
boundaries.

Removing dynamic initialization made query borrowing predictable, but it
forbade defaults whose value depended on whichever world state happened to
exist at first access. Callers instead had to compute such values explicitly
when creating entities, potentially through higher-level or bulk construction
code.

## Alternatives considered

### Keep separate property tag and value types

The existing representation allowed a single value type to be reused by
several property tags, including primitive types. It was rejected because it
duplicated the identity of a property, produced awkward tag arguments and
`PropertyValue` naming, and weakened type inference in normal property access.

### Store entities in the generic data-plugin registry

Entities could have reused a single registry abstraction for all
plugin-discovered context data. A separate registry was selected because
entities had distinct population counting, property storage, lazy
initialization, and access requirements. Keeping a dedicated `EntityStore`
also allowed those semantics to evolve without changing unrelated context
data.

### Retain dynamic initialization and expose mutable and immutable compute APIs

One proposal added a separate `compute_immutable` operation, with macros
responsible for ensuring that it never mutated. This made the query contract
explicit, but enlarged a fundamental property API and relied on implementations
honoring a convention the type system did not enforce.

### Retain dynamic initialization with fast and slow read paths

Another proposal first attempted an immutable read, dropped its borrows if
initialization was needed, and then retried through a mutable path. This avoided
a second public compute API, but duplicated access work, exposed storage details
to property implementations, and still relied on every query performing the
mutating phase at the correct time. Because dynamic defaults were not needed in
practice and their first-read semantics were themselves difficult to reason
about, explicit creation-time initialization was preferred.

## References

- [ADR-0002: Represent query results with `EntitySet`](0002-represent-query-results-with-entity-set.md)
- [ADR-0003: Maintain property indexes eagerly without `RefCell`](0003-maintain-property-indexes-eagerly.md)
- [GitHub issue #389: feat: Entities](https://github.com/CDCgov/ixa/issues/389)
- [`7ef37d5`: `feat: Entities implementation`](https://github.com/CDCgov/ixa/commit/7ef37d57279d0206018470b0ec8be3d585e7f01a)
- Feature branch: `RobertJacobsonCDC_entities`

The principal adoption commit is a consolidated change on `main` with no pull
request number in its subject. Its author date is 2025-11-11, but it entered
the current first-parent history on 2026-02-09; this ADR uses the latter as the
best-supported adoption date. The retained feature branch records development
from November 2025 through January 2026 but is not a direct ancestor of the
consolidated `main` commit.

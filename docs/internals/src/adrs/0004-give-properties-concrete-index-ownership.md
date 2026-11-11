# ADR-0004: Give each property concrete index ownership

| Field | Value |
| --- | --- |
| Decision date | 2026-06-15 (merge of PR #937) |
| Recorded | 2026-07-29 |
| Status | Accepted |
| GitHub issue | [#909](https://github.com/CDCgov/ixa/issues/909) |
| Pull request | [#937](https://github.com/CDCgov/ixa/pull/937) |
| Feature branch | `RobertJacobsonCDC_remove_shared_indexes` |

## Summary

Ixa stopped allowing one property to own or use an index stored by another
property. Each concrete property `P` became the sole owner of the index in its
own `PropertyValueStoreCore&lt;E, P&gt;`, addressed by `Property::id()`.
`Property::index_id()` and the macro hooks that could redirect index ownership
were removed.

Equivalent multi-properties remained query-equivalent but no longer shared
storage. An entity-scoped registry selected one representative for each
unordered component set, and queries in any component order resolved to that
representative. Later equivalent definitions kept distinct storage and property
IDs; Ixa warned about them and rejected attempts to create indexes that normal
query resolution could never reach.

## Context

A multi-property is a derived tuple property used to index several component
properties jointly. For example, `(Age, Vaccinated)` and `(Vaccinated, Age)`
are distinct Rust tuple types, but a query over `Age` and `Vaccinated` should
use the same joint index regardless of the order in which the query terms are
written.

The earlier design implemented that order independence by sharing index
ownership. `Property::id()` identified a property's concrete storage, while
`Property::index_id()` could redirect index operations to an equivalent
property's store. Multi-property macros chose a common index ID, and generic
code used that ID when enabling, testing, populating, and querying an index.

This conflated two different concepts:

- **Concrete ownership:** which `PropertyValueStoreCore&lt;E, P&gt;` stores and
  maintains an index for the concrete property `P`.
- **Logical query identity:** which registered multi-property represents an
  unordered component set during query planning.

The aliasing meant that an operation parameterized by `P` could mutate a
different property's type-erased store. It complicated reasoning about index
maintenance and made `is_property_indexed::&lt;P&gt;()` report the state of an
equivalent property rather than `P` itself. It also blocked a later static
separation between ordinary properties and properties supporting the
`Eq + Hash` requirements of indexing.

The replacement needed to preserve order-insensitive query behavior. Merely
giving every equivalent multi-property an independent index would be wasteful
and ambiguous: ordinary queries over the component set still needed one
deterministic property ID to consult.

## Decision

Ixa separated concrete index ownership from logical query routing.

### Concrete ownership

`Property::id()` became the only storage and index-owner ID for a concrete
property:

- `Property::index_id()` was deleted.
- The internal `index_id_fn` macro option and its supporting expansion paths
  were deleted.
- Index creation, index-state inspection, initial population, and subsequent
  maintenance operated on `P::id()` and
  `PropertyValueStoreCore&lt;E, P&gt;`.
- `is_property_indexed::&lt;E, P&gt;()` answered only whether the concrete property
  `P` owned an index.

An ordinary property and every concrete multi-property therefore had distinct,
non-aliased index storage.

### Representative query routing

For each entity type and unordered multi-property component set, registration
selected the first registered concrete multi-property as the representative.
Query resolution used that representative's property ID without transferring
ownership of its index to equivalent types.

The representative registry was scoped by entity identity because numeric
property IDs were local to an entity type. It supported two lookup shapes:

1. `(entity ID, sorted component type IDs) -> representative`, used when a
   query was expressed as separate component properties.
2. `(entity ID, logical multi-property type ID) -> representative`, used by
   generic code that had a concrete `P: Property&lt;E&gt;` but no component list.

The second lookup handled singleton queries whose one term was itself a
multi-property, logical source identity inside `SourceSet`, and the guard
against indexing a non-representative duplicate. Recording both keys during
macro-generated registration avoided adding multi-property component metadata
to the general `Property` trait.

### Duplicate equivalent multi-properties

Later registrations with the same entity and unordered component set remained
legal. They received their own `Property::id()` and storage, but the registry
kept the original representative.

Because queries for that component set always routed to the representative, an
index placed on a duplicate would be inaccessible through normal query
resolution. Ixa therefore:

- queued a warning when startup registration found a duplicate;
- emitted queued warnings when the first `Context` was constructed, after
  logging could be initialized; and
- panicked if client code attempted to index the duplicate rather than the
  representative.

Defining only one multi-property for an unordered component set became the
recommended client practice.

### Decision boundary

The ownership refactor was the first of three commits on the feature branch.
Its initial implementation deliberately retained `Property::CanonicalValue`
and the universal `Property: Eq + Hash` requirement so those public API changes
could be evaluated separately. The following branch commits removed canonical
value machinery and restricted `Eq + Hash` to indexable properties; all three
changes merged together in PR #937. Those follow-up decisions are recorded
separately as
[ADR-0005](0005-key-indexes-by-concrete-property-values.md) and
[ADR-0006](0006-restrict-eq-hash-to-indexable-properties.md).

## Rationale

Making `P::id()` the sole owner ID restored the expected relationship between a
generic property parameter and the storage being accessed. Index construction
could statically fetch `PropertyValueStoreCore&lt;E, P&gt;`, property-change handling
could update only that store, and index-state checks acquired concrete rather
than equivalence-based semantics.

Separating routing from ownership preserved the useful behavior of shared
indexes without sharing storage. Queries over the same components continued to
resolve identically in any term order, while only one concrete property could
own the index used for that logical query.

The two representative maps reflected information available at different call
sites. Query tuples could derive an unordered component key but had no concrete
multi-property type. Generic `P`-based code had a logical property type ID but
no general way to recover `P`'s components. Registering both mappings once was
smaller and more localized than expanding the `Property` trait or rebuilding
macro-specific metadata during every lookup.

Rejecting indexes on duplicates prevented silent waste and misleading state.
Allowing the duplicate definition itself preserved compatibility and supported
diagnostics, while making the unusable operation fail explicitly.

## Consequences

- Every index had one concrete, type-aligned owner. Index creation and
  maintenance no longer redirected through `Property::index_id()`.
- Equivalent multi-properties retained distinct property IDs and storage while
  remaining equivalent for query routing.
- Queries written with the same component set in different orders continued to
  use the representative's index.
- Index-state inspection became concrete: indexing the representative no longer
  made a duplicate report itself as indexed.
- The change removed the public/internal `Property::index_id()` customization
  surface, including `index_id_fn` macro support.
- The representative registry became more explicit and more complex, with two
  entity-scoped lookup maps and a distinction between concrete storage identity
  and logical query identity.
- When equivalent multi-properties were defined more than once, startup
  constructor order determined which one became representative. Client code
  could not rely on choosing among duplicates, so Ixa warned and recommended
  defining only one.
- Pre-`main` duplicate detection required warnings to be queued until context
  construction, adding a small diagnostic lifecycle mechanism.
- Concrete ownership enabled the subsequent removal of canonical-value
  indirection and the introduction of a narrower `IndexableProperty`
  requirement, but those were separate decisions.

## Alternatives considered

### Retain shared ownership through `Property::index_id()`

The existing approach guaranteed that equivalent multi-properties referred to
one index and avoided duplicate-storage questions. It was rejected because a
property operation could target another property's store, concrete index state
was obscured, and static property/index bounds could not be expressed cleanly.

### Give every equivalent multi-property an independently usable index

This would have made ownership concrete without selecting a representative.
However, queries over separate component properties still needed to choose an
index. Supporting several interchangeable indexes would add selection policy
and maintenance cost; choosing one while allowing the others would create
indexes that ordinary queries could not reach. Ixa instead permitted duplicate
definitions but prohibited indexing non-representatives.

### Make query routing order-sensitive

Treating `(Age, Vaccinated)` and `(Vaccinated, Age)` as unrelated would remove
the representative registry. It was rejected because query term order is not a
meaningful semantic distinction and existing multi-property behavior promised
order-insensitive lookup.

### Use one representative map for every lookup

The component-set map could not serve generic `P`-based code without exposing
multi-property components through `Property`. Conversely, a concrete
property-type map could not serve query tuples made from separate component
properties. Keeping both keys in the registration data localized this
multi-property-specific knowledge and avoided expanding the general trait.

## References

- [GitHub issue #909](https://github.com/CDCgov/ixa/issues/909)
- [PR #937](https://github.com/CDCgov/ixa/pull/937)
- [Feature-branch commit `41bc5d3`: remove shared index ownership](https://github.com/CDCgov/ixa/commit/41bc5d3baf55ef92cd99c0a416c7f0124d4628c8)
- [Adopted `main` commit `7b59c16`](https://github.com/CDCgov/ixa/commit/7b59c16bbc8ffdbb0e3e63186652ffa3bace6f3d)
- Feature branch: `RobertJacobsonCDC_remove_shared_indexes`

The feature branch contains three ordered commits for the three decisions that
were squashed into `7b59c16` on `main`. The branch-tip tree and merged tree
match exactly, while `41bc5d3` isolates this ADR's ownership change before the
two follow-up refactors.

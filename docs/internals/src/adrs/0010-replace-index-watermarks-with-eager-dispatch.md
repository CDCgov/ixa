# ADR-0010: Replace index watermarks with eager new-entity dispatch

| Field | Value |
| --- | --- |
| Decision date | 2026-07-27 (merge of PR #1009) |
| Recorded | 2026-07-29 |
| Status | Accepted |
| Pull request | [#1009](https://github.com/CDCgov/ixa/pull/1009) |
| Feature branch | `RobertJacobsonCDC_refactor_add_entity` |

## Summary

Ixa replaced per-index population watermarks with an invariant that every
installed property index was complete. A new or replacement index was built
and populated while detached, then installed only after construction
succeeded. Every later entity was inserted exactly once into each active index
through a compact list of typed function-pointer dispatchers.

This refined the eager-maintenance architecture recorded in
[ADR-0003](0003-maintain-property-indexes-eagerly.md). It removed the remaining
catch-up machinery and the raw-pointer aliasing used by entity creation and
index configuration, made replacement panic-safe, and kept `add_entity` work
proportional to the number of active indexes rather than all registered
properties.

## Context

ADR-0003 had moved index maintenance from query-time lazy updates to entity and
property mutation paths. Its adopted implementation still associated every
full and value-count index with `max_indexed`, a population watermark. Enabling
an index installed it first and then caught it up to the existing population.
Creating an entity wrote its explicit values and asked the property store to
catch up every indexed property from its watermark to the new population size.

That mechanism kept enabled indexes eager from a caller's perspective, but an
index could be temporarily installed while incomplete. Replacing an index
could discard the old valid index before the replacement had successfully
computed every value. A panic during derived-property computation or
allocation could therefore expose partial index and lifecycle state.

The catch-up paths also needed both immutable property access through
`&amp;Context` and mutable access to index storage. `add_entity`,
`index_property`, and `index_property_counts` created a raw pointer to the
context and reconstructed a shared reference inside an `unsafe` block while a
property store was mutably borrowed. The intended accesses were disjoint, but
the abstraction could not express the sequencing to Rust.

Entity creation was a hot path. Iterating all registered type-erased property
stores to discover which ones had indexes made its index-maintenance work scale
with registered properties, including unindexed ones. The refactor instead
needed to scale with active indexes without allocating or cloning a dispatcher
collection for each entity.

Full and value-count indexes also required a coherent replacement policy.
Repeated value-count insertion is not idempotent, so a transitional design
that both populated a new index eagerly and retained watermark catch-up could
double-count the existing population. The new lifecycle therefore had to
replace the old one as a single behavioral cutover.

## Decision

### Build a complete index before installation

Both `index_property` and `index_property_counts` delegated to one typed index
creation operation. It:

1. validated that a multi-property was the registered representative;
2. inspected whether the installed index already satisfied the request;
3. constructed an empty full or value-count index as a local trait object;
4. populated it from every existing entity using immutable context access; and
5. installed the completed index and its dispatcher state together.

The index remained unreachable through `Context` while it was populated.
Property computation could therefore borrow the context immutably while the
new index was mutated locally. If validation, allocation, property
computation, or insertion panicked, the local replacement was dropped and the
previous installed index and dispatcher remained unchanged.

Index requests followed these rules:

- repeating the currently installed kind was a no-op;
- a full index satisfied a value-count request and was left unchanged;
- a value-count index was fully rebuilt as a full index when a full index was
  requested; and
- an unindexed property received a fully populated index of the requested
  kind.

Concrete index construction remained in the index subsystem through a factory
on `PropertyIndexType`. Context-level request policy handled only index kinds
and a boxed `PropertyIndex`, not `FullIndex` or `ValueCountIndex` directly.

### Dispatch new entities only to active indexes

Each entity-specific `PropertyStore` maintained
`index_new_entity_fns`, with exactly one `(property_id, function_pointer)`
entry for each actively indexed property. The property ID was the stable
dispatcher identity; function addresses were not used because link-time
deduplication could make pointer identity unreliable.

Installing the first index for a property reserved dispatcher capacity before
the index commit, then installed the index and appended its typed dispatcher.
Replacing one indexed representation with another retained the existing
dispatcher. Removing an index removed its dispatcher by property ID. These
coupled updates were centralized in `install_property_index`.

`add_entity` adopted this ordering:

1. validate the initialization list and allocate the entity ID;
2. write all explicitly supplied values;
3. invoke each active index dispatcher exactly once; and
4. emit `EntityCreatedEvent`.

Each dispatcher first computed and copied its property's final value through
immutable context access. It then ended that borrow, obtained mutable access to
the installed typed property index, and inserted the entity. This sequencing
supported explicit, default, derived, and multi-property values without an
aliased context reference. Because all explicit values were already written,
derived and multi-property computations observed the complete initialized
entity, and entity-created handlers observed updated indexes.

The dispatch loop copied one bare function pointer at a time before passing
`&amp;mut Context` to it. It did not clone the active list, allocate a temporary
collection, or probe unindexed property stores.

### Remove the watermark lifecycle

`max_indexed` and its accessors were removed from both concrete index types and
the `PropertyIndex` trait. The type-erased catch-up methods, the
all-properties catch-up traversal, and the old empty-index installation path
were deleted.

The resulting invariant was direct: an installed index contained the full
existing population, and entity creation updated every installed index before
the new entity became observable through `EntityCreatedEvent`.

This change deliberately did not alter `PropertyList`, property-change event
semantics, value-change counters, or property-initialization event behavior.
Events for explicitly initialized properties were separate follow-on work.

## Rationale

Detached construction turned index configuration into a validate, build, and
commit operation. It eliminated the borrow conflict that had motivated the
remaining `unsafe` blocks and strengthened failure behavior: the old valid
index stayed available until a complete replacement existed.

An active dispatcher list moved type erasure and discovery to the cold index
configuration path. Entity creation then paid traversal overhead only for
indexes that required an insertion. Bare monomorphized function pointers
preserved typed property access without captured references or per-entity
dispatcher allocation.

Writing explicit values before dispatch established one clear point at which a
new entity's properties were complete enough to index. It avoided indexing a
derived or multi-property from a partially initialized entity and ensured that
the subsequent creation event did not expose stale indexes.

The shared installation operation kept index presence and dispatcher presence
as one invariant. Reserving capacity before the commit avoided an allocation
failure after replacing a valid index, while retaining the dispatcher across
indexed-to-indexed replacement avoided duplicates.

The feature introduced a dedicated seven-case `add_entity` benchmark before
the production refactor. It compared zero, one, and three active indexes;
full and value-count indexes; omitted, explicit-default, and non-default
initialization; and narrow versus wide registered-property sets. This made the
expected active-index scaling and normal index-allocation costs measurable.
The retained repository evidence does not include benchmark results, so this
record makes no measured performance claim.

## Consequences

- Every installed index represented the full entity population; no watermark
  or later catch-up step was needed.
- Enabling or replacing an index performed all population work before
  installation. During replacement, the old and new index could coexist,
  temporarily increasing memory use.
- A failed replacement preserved the old index and its dispatcher rather than
  leaving a partially populated index installed.
- New-entity index traversal scaled with the number of active indexes, not the
  number of registered properties. Concrete index insertions could still
  allocate as their maps and entity sets grew.
- Each indexed property added one dispatcher and therefore one property-value
  computation and insertion to every subsequent entity creation.
- Default, derived, and multi-property indexes observed the completed
  initialized state even when their values were not explicitly stored in the
  initialization list.
- `EntityCreatedEvent` callbacks could query enabled indexes and find the new
  entity.
- Repeated index requests became explicitly idempotent, and upgrading a
  value-count index to a full index preserved existing entities and one active
  dispatcher.
- Index configuration and `add_entity` no longer constructed an aliased
  `&amp;Context` through a raw pointer; the `unsafe` mechanism retained by the
  earlier implementation was removed from all three call sites.
- Correctness depended on maintaining the one-index/one-dispatcher invariant
  through the centralized installation operation.
- The benchmark target added maintenance cost but provided controlled coverage
  of the hot path and its intended scaling properties.

## Alternatives considered

### Retain watermark-based eager catch-up

The existing design already kept indexes current before public observation in
ordinary execution. Retaining it would have avoided the refactor, but it kept
temporarily incomplete installed indexes, all-property catch-up traversal, and
the raw-pointer borrowing workaround. It also made replacement failure less
atomic.

### Install an empty index and populate it in place

This used less temporary memory than keeping the old and new indexes
simultaneously. It was not selected because population needed immutable context
access while installed index storage was mutably borrowed, recreating the
aliasing problem. A panic could also leave the installed replacement partial.

### Probe every registered property during entity creation

Each type-erased property store could have exposed an optional new-entity
operation. That would have kept dispatcher state out of index installation,
but every entity creation would pay a virtual call or pointer chase for every
registered property. The active list instead moved that discovery cost to rare
index configuration.

### Clone or materialize the dispatcher list before invocation

Owning a temporary dispatcher collection would make the borrow boundary
obvious, but it would allocate or copy the collection on the entity-creation
hot path. Copying one function pointer into a local variable ended the
immutable store borrow without per-entity collection construction.

### Introduce eager dispatch incrementally alongside watermarks

An additive rollout could have installed fully populated indexes while leaving
the old catch-up path active. That was rejected as an invalid runtime state:
the existing population could be reinserted, and value-count indexes would
silently overcount. Detached installation, active dispatch, and watermark
removal were therefore one atomic behavioral cutover.

## References

- [ADR-0003: Maintain property indexes eagerly without `RefCell`](0003-maintain-property-indexes-eagerly.md)
- [PR #1009: Refactor `add_entity` and index maintenance](https://github.com/CDCgov/ixa/pull/1009)
- [Benchmark commit `6df0c86`](https://github.com/CDCgov/ixa/commit/6df0c860cd58d33700249890b5177314e5276961)
- [Production refactor commit `3f9cc03`](https://github.com/CDCgov/ixa/commit/3f9cc03bbb9e09c7c9da874abeaf720d97f12ae7)
- [Adopted squashed commit `33e56d4`](https://github.com/CDCgov/ixa/commit/33e56d44d4db000fb0898af5c5e1aba3e212db58)

PR #1009 squashed the two retained feature-branch commits. All changed feature
paths other than `context_extension.rs` match between the branch tip and
merged commit; the differences in that file are unrelated `Into&lt;f64&gt;`
scheduling API changes that reached `main` before the squash. The
implementation plan in `Notes/plan-add_entity-refactor.md`, the merged code,
regression tests, and benchmark target supplied the reconstruction evidence
for this record.

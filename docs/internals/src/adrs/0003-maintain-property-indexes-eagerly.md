# ADR-0003: Maintain property indexes eagerly without `RefCell`

| Field | Value |
| --- | --- |
| Decision date | 2026-03-17 (merge of PR #799) |
| Recorded | 2026-07-29 |
| Status | Accepted |
| GitHub issue | [#769](https://github.com/CDCgov/ixa/issues/769) |
| Pull request | [#799](https://github.com/CDCgov/ixa/pull/799) |
| Feature branch | `RobertJacobsonCDC_remove_refcell_for_index` |

## Summary

Ixa removed `RefCell` from full and value-count property indexes and changed
index maintenance from lazy query-time catch-up to eager write-time updates.
Index reads returned ordinary shared references, index writes required mutable
references, and an enabled index was intended to be current before it could be
read.

The decision simplified query borrows and index ownership, consolidated index
updates with entity and property writes, and produced a small, consistent
performance improvement in local benchmarks. At adoption, it also accepted two
narrow uses of `unsafe` to reconcile mutable access to a property store with
immutable context access needed to compute derived values. A later refactor
removed those uses without reversing the eager-maintenance decision.

## Context

Ixa's initial entity indexes were lazy. `PropertyIndex` stored `FullIndex` and
`ValueCountIndex` values inside `RefCell`, allowing query paths with only
`&amp;Context` to notice that an index lagged behind the population and mutate it
before reading. Each index tracked a population watermark so it could process
only entities added since its previous use.

This design had originally been expected to save work when an indexed property
changed repeatedly without being queried. In practice, local benchmarks did not
show the expected advantage; the eager prototype was instead consistently, if
only slightly, faster. Performance alone was not considered sufficient reason
for the change.

Interior mutability also affected the query-result types introduced in
[ADR-0002](0002-represent-query-results-with-entity-set.md). An indexed source
had to carry a `Ref&lt;IndexSet&lt;T&gt;&gt;` borrow guard instead of an ordinary
`&amp;IndexSet&lt;T&gt;`. This complicated reference types throughout `EntitySet` and
`EntitySetIterator` and prevented straightforward cloning of structures that
contained those sources.

Lazy catch-up spread index maintenance across query lookup, entity creation,
property storage, and partial property-change events. Index APIs accepted a
`Context` solely so a read could perform hidden mutation, and `RefCell`
enforced borrow compatibility at runtime rather than through normal mutable
references.

## Decision

Ixa adopted eager maintenance for every enabled property index and removed
`RefCell` from `PropertyIndex`:

- `FullIndex` and `ValueCountIndex` were stored directly in the enum.
- Read operations took `&amp;self` and returned plain references or counts.
- Operations that inserted or removed entity IDs took `&amp;mut self`.
- Query-time index lookup stopped performing catch-up and no longer accepted a
  context parameter for that purpose.

At adoption, index consistency was maintained at the following mutation
boundaries:

1. Enabling a full index constructed it and immediately caught it up to the
   current entity population.
2. Adding an entity wrote its initial properties, then caught up every enabled
   index for that entity type before emitting `EntityCreatedEvent`.
3. Setting a property first snapshotted the previous values of that property
   and its derived dependents. After writing the new value, partial-event
   emission removed each entity ID from its old index bucket, inserted it into
   the new bucket, and emitted the corresponding property-change event.

The March implementation retained population watermarks as its mechanism for
catching up indexes during index creation and entity creation. The invariant
changed from "a query catches up an index before reading it" to "mutation paths
keep an enabled index current before any query can observe it."

### Accepted safety trade-off

Two paths needed to mutate an index while computing property values through an
immutable context reference:

- `ContextEntitiesExt::add_entity`, when updating enabled indexes after initial
  values had been written; and
- `ContextEntitiesExt::index_property`, when building an index for an existing
  population.

Derived properties required `P::compute_derived(&amp;Context, entity_id)`, while the
index being populated was reached through a mutable borrow of the context's
property store. Rust could express disjoint borrows of fields locally, but the
abstraction crossed method boundaries that appeared to borrow the whole
context. The adopted implementation used a raw context pointer to create a
shared reference during these two calls. The intended aliasing boundary was
narrow: the shared reference was for property reads and derived computation,
while the mutable reference was used only for index internals.

## Rationale

Ordinary references made the ownership model explicit. The compiler could
distinguish index reads from writes, and query code no longer carried runtime
borrow guards or invoked hidden mutation. Removing lazy-era context parameters
and catch-up calls also made the index lookup path smaller and easier to
review.

Eager maintenance placed consistency work next to the mutations that caused it.
All old-value removal and new-value insertion for a property change occurred in
one phase instead of being split between partial-event construction and
emission. Queries could then rely on an already-current index.

The approach also made index-backed source sets hold ordinary shared
references. This removed the `RefCell`-specific obstacle to cloning
`EntitySet` and `EntitySetIterator`; clone support itself was added shortly
afterward in PR #837.

The local benchmark results were supporting rather than decisive evidence.
They showed no payoff from lazy maintenance and a small consistent improvement
from the eager prototype, so performance did not justify retaining the more
complex ownership model.

## Consequences

- Index reads became ordinary immutable operations with no runtime borrow
  checks or query-time writes.
- Entity creation and property changes paid index-maintenance costs immediately,
  even if the affected index was never queried afterward.
- Query and set-expression code used `&amp;IndexSet&lt;T&gt;` rather than
  `Ref&lt;IndexSet&lt;T&gt;&gt;`, simplifying lifetimes and enabling later clone support.
- Index consistency depended on every relevant mutation path performing its
  eager update. This made the invariant clearer, but omissions became
  correctness bugs rather than deferred work a query could repair.
- The March implementation accepted narrowly scoped `unsafe` code and retained
  watermark-based catch-up, adding a safety and maintenance obligation.

One such omission existed at adoption: `index_property_counts` enabled a
value-count index without backfilling entities already in the context. PR #1007
fixed that gap on 2026-07-16 by eagerly populating the value-count index when it
was enabled.

[ADR-0010](0010-replace-index-watermarks-with-eager-dispatch.md) subsequently
replaced the watermark and raw-pointer mechanism. Indexes were built while
detached and installed only after successful population; registered
type-specific dispatchers updated enabled indexes for new entities. This
removed the two `unsafe` blocks and strengthened failure handling while
preserving eager index maintenance, so it refined rather than superseded this
decision.

## Alternatives considered

### Retain lazy indexes inside `RefCell`

Lazy catch-up could avoid maintenance work between queries and avoided the
borrow conflict that led to the two `unsafe` blocks. It was rejected because
the expected performance benefit did not appear in local measurements and
because it imposed runtime borrow guards, hidden mutation, and additional
plumbing on every index-backed query.

### Restructure index construction to avoid `unsafe` immediately

The team could have redesigned the context/property-store boundary so derived
values were computed before mutable index access, or dispatched updates through
type-specific functions. That required a larger lifecycle and ownership
refactor than PR #799. The narrow raw-pointer approach was accepted as the
simplest implementation at the time; PR #1009 later completed the broader safe
refactor.

## References

- [Issue #769: Evaluate eager vs. lazy indexing strategy](https://github.com/CDCgov/ixa/issues/769)
- [PR #799: Remove `RefCell` from indexes](https://github.com/CDCgov/ixa/pull/799)
- [Feature-branch commit `14d525b`](https://github.com/CDCgov/ixa/commit/14d525b277bcce7284fb3553b5c233804c97b0e6)
- [Adopted commit `45a1090`](https://github.com/CDCgov/ixa/commit/45a10907391173ba67c724367cc3fa9b8f81851e)
- [PR #837: Add clone support for `EntitySet`](https://github.com/CDCgov/ixa/pull/837)
- [PR #1007: Eagerly compute property value-count indexes](https://github.com/CDCgov/ixa/pull/1007)
- [ADR-0010: Replace index watermarks with eager new-entity dispatch](0010-replace-index-watermarks-with-eager-dispatch.md)
- [PR #1009: Refactor `add_entity` and index maintenance](https://github.com/CDCgov/ixa/pull/1009)

The retained feature-branch commit and the adopted `main` commit contain the
same index refactor. Their complete trees differ because unrelated global
property changes reached `main` before PR #799 merged.

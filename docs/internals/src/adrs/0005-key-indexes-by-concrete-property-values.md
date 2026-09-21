# ADR-0005: Key indexes by concrete property values

| Field | Value |
| --- | --- |
| Decision date | 2026-06-15 (merge of PR #937) |
| Recorded | 2026-07-29 |
| Status | Accepted |
| GitHub issue | [#909](https://github.com/CDCgov/ixa/issues/909) |
| Pull request | [#937](https://github.com/CDCgov/ixa/pull/937) |
| Feature branch | `RobertJacobsonCDC_remove_shared_indexes` |

## Summary

Ixa removed the `Property::CanonicalValue` associated type and its conversion
API. Full and value-count indexes were keyed directly by the concrete property
value `P`, and index lookup reconstructed a `P` from type-erased query parts
instead of converting those parts to a separate canonical key type.

For multi-properties, order-insensitive query routing remained a responsibility
of the runtime multi-property registry. Component query parts were ordered by
their Rust `TypeId`s and then reconstructed in the indexed multi-property's
declared tuple order. This kept equivalent component queries working without
making canonical values part of the property or index model.

## Context

`Property&lt;E&gt;` previously had an associated `CanonicalValue` type together with
`make_canonical`, `make_uncanonical`, and
`canonical_from_sorted_query_parts`. Ordinary properties normally used
themselves as their canonical type. The public API also allowed a property to
declare a different indexed representation and conversions between the two.

Multi-properties depended heavily on that machinery. A tuple such as
`(Age, Weight)` had to match a query or equivalent multi-property whose
components appeared in a different order. Generated code converted each tuple
to a name-sorted canonical tuple so an index lookup had one common key
representation. Supporting procedural macros generated the sorted tuple type
and the reorder and inverse-reorder functions.

The preceding change on the same feature branch,
[ADR-0004](0004-give-properties-concrete-index-ownership.md), had already
stopped equivalent multi-properties from sharing index ownership. Each
concrete property owned its own index, while a runtime registry selected one
representative multi-property for an unordered component set. Canonical values
therefore no longer solved an index-ownership problem; they remained as an
additional key type and conversion layer within each concrete index.

The general canonical-value feature had no known use in the workspace outside
documentation, tests, benchmarks, and the multi-property implementation.
Meanwhile, its name-based multi-property ordering forced
`define_multi_property!` to reject component type aliases, because a source
alias name could differ from the underlying property's registered name.

This decision was the middle of three deliberately sequenced changes on
`RobertJacobsonCDC_remove_shared_indexes`: remove shared index ownership, remove
canonical values, then
[restrict `Eq + Hash` to indexable properties](0006-restrict-eq-hash-to-indexable-properties.md).
The three changes merged together in PR #937, but each addressed a distinct
architectural constraint.

## Decision

Ixa made the concrete property value the index key:

- `FullIndex&lt;E, P&gt;` stored `HashMap&lt;P, IndexSet&lt;EntityId&lt;E&gt;&gt;&gt;`.
- `ValueCountIndex&lt;E, P&gt;` stored `HashMap&lt;P, usize&gt;`.
- Index creation, lookup, insertion, removal, and property-change maintenance
  accepted `&amp;P` directly.
- `Property::CanonicalValue`, `make_canonical`, `make_uncanonical`, and
  `canonical_from_sorted_query_parts` were removed.
- The corresponding public macro options and the procedural macros that
  generated canonical tuple types and reorder functions were removed.

`Property&lt;E&gt;` instead provided
`value_from_query_parts(parts: &amp;[&amp;dyn Any]) -&gt; Option&lt;Self&gt;`. For an ordinary
property, this downcast the single query part to `Self`. For a multi-property,
generated code:

1. ordered component identities by `TypeId`;
2. computed the inverse mapping from that order to the multi-property's
   declared tuple order;
3. downcast each type-erased query part to its component property type; and
4. constructed the concrete tuple value `P` used by the index.

Tuple-query implementations used the same component `TypeId` ordering as the
multi-property registry. Equivalent multi-properties retained distinct
concrete property identities; equivalence was represented only by the registry
that routed an unordered component set to its representative property.

Because routing and reconstruction depended on the underlying component types
rather than their source-level spelling, `define_multi_property!` allowed type
aliases again.

The global `Property: Eq + Hash` requirement was intentionally retained during
this step because indexes were now hash maps keyed by `P`. The immediately
following branch change introduced the narrower `IndexableProperty` bound; that
was a separate decision.

## Rationale

Using `P` as the key aligned index storage with the property store and mutation
paths. Index updates no longer transformed stored or computed property values
into a second representation, and lookup no longer had to reverse that
representation. Removing the associated type also reduced the public property
API and eliminated substantial macro machinery maintained primarily for
multi-properties.

Runtime type identity was a better ordering basis for multi-property
components than source-level names. The registry already used sorted component
`TypeId`s to resolve an unordered query to a representative multi-property.
Using the same ordering for query values made routing and value reconstruction
consistent and allowed aliases to behave like their underlying types.

Keeping equivalence in the registry preserved order-insensitive queries without
claiming that differently declared tuple properties were the same concrete
Rust type. Each index could therefore remain typed and keyed by its owning
property.

## Consequences

- Property indexes stored and compared the same concrete values exposed by the
  property API.
- Property authors no longer had to understand or implement a canonical key
  type and bidirectional conversions.
- The public canonical-value customization was removed as a breaking API
  change. A property could no longer expose one value type while indexing a
  transformed value type.
- Multi-property query routing remained order-insensitive, but lookup required
  runtime `TypeId` ordering, inverse-index calculation, and type-erased
  downcasts to reconstruct the representative tuple.
- Multi-property component aliases became supported because routing no longer
  depended on source-level names.
- Equivalent multi-properties had distinct concrete `Property::type_id()`
  values; the registry, rather than a shared canonical tuple identity,
  represented their query equivalence.
- Removing canonical values made it possible for the next branch step to place
  `Eq + Hash` requirements only on properties that were actually indexed.

The `a1c7d51` branch commit temporarily added a hash of logically equivalent
query values to support structural comparison of property-backed
`EntitySet`s. The immediately following `Eq + Hash` cleanup replaced that hash
with comparison through `value_from_query_parts`. Thus the hash helper was not
part of the final tree merged by PR #937, while direct index keys and
TypeId-based reconstruction were.

## Alternatives considered

### Retain canonical values as a public property feature

This would have preserved custom transformed index keys and avoided a breaking
API removal. It was not selected because no workspace use justified the
associated type, conversions, documentation, testing, and macro complexity
once multi-property index ownership had been separated.

### Keep canonical tuples only for multi-properties

Canonical tuples could have remained an internal mechanism for
order-independent lookup even after the general public feature was removed.
The runtime representative registry already supplied the necessary logical
routing, however, and `TypeId`-ordered query parts could reconstruct the
concrete tuple expected by the owning index. A second tuple type and conversion
layer were unnecessary.

### Continue ordering multi-property components by source name

Name ordering was familiar from the existing canonical tuple implementation,
but aliases made the spelling at a macro call site differ from the underlying
property name. Retaining it would have kept the alias prohibition and used
diagnostic text as a runtime identity mechanism. `TypeId` ordering matched the
registry's existing type-based lookup.

### Combine this change with the property trait-bound split

Direct `P` keys exposed exactly where `Eq + Hash` was required and made the
follow-up possible. Combining both transformations in one branch commit would
have obscured whether failures came from key representation or trait-bound
changes. The work was kept as consecutive commits even though both ultimately
merged in PR #937.

## References

- [Issue #909](https://github.com/CDCgov/ixa/issues/909)
- [PR #937: Replace the universal property `Eq + Hash` constraint](https://github.com/CDCgov/ixa/pull/937)
- [Commit `41bc5d3`: preceding removal of shared index ownership](https://github.com/CDCgov/ixa/commit/41bc5d3baf55ef92cd99c0a416c7f0124d4628c8)
- [Commit `a1c7d51`: remove `Property::CanonicalValue` and supporting machinery](https://github.com/CDCgov/ixa/commit/a1c7d519807876a74f447cd7c1dcce12e27a0f5e)
- [Commit `4e914d8`: following property trait-bound split](https://github.com/CDCgov/ixa/commit/4e914d8f32ea8adef979c8581a6a8a4a1fc0a325)
- [Merged commit `7b59c16`](https://github.com/CDCgov/ixa/commit/7b59c16bbc8ffdbb0e3e63186652ffa3bace6f3d)

The tree at the feature-branch tip `4e914d8` matches the merged tree at
`7b59c16`. The branch commits provide the clearest traceability for the three
decisions delivered together by PR #937.

# ADR-0002: Represent query results with `EntitySet`

| Field | Value |
| --- | --- |
| Decision date | 2026-02-27 (merge of PR #786) |
| Recorded | 2026-07-29 |
| Status | Accepted |
| GitHub issue | [#746](https://github.com/CDCgov/ixa/issues/746) |
| Pull requests | [#745](https://github.com/CDCgov/ixa/pull/745), [#786](https://github.com/CDCgov/ixa/pull/786) |
| Feature branches | `RobertJacobsonCDC_746_entity_set`; precursor `RobertJacobsonCDC_v2.0.0_rename_query_result_iterator` |

## Summary

Ixa made `EntitySet&lt;E&gt;` the public representation of reusable entity-query
results and retained `EntitySetIterator&lt;E&gt;` as the streaming execution form.
An `EntitySet` represented a lazy set expression over entity IDs rather than an
eagerly materialized collection. It supported membership tests, iteration, and
composition through union, intersection, and difference while hiding whether
its leaves came from the population, an index, or property-backed filtering.

This separated the meaning of a query from the representation and execution of
its result: `Query` described what should match, `EntitySet` represented the
resulting set, and `EntitySetIterator` executed the expression lazily.

## Context

Before this decision, the public query path was centered on iteration.
`QueryResultIterator` streamed matching entity IDs, while
`with_query_results` could expose the concrete `IndexSet` used by an indexed
query. The latter did not generalize well: future indexes could use different
containers, and an unindexed query had to realize a complete result merely to
provide the same callback-shaped API.

The underlying query sources were already broader than queries themselves.
Population ranges, index buckets, and property-backed sources all had set-like
semantics: they could test membership and iterate unique entity IDs. The
February 9 precursor change moved these types into the `entity_set` module and
renamed `QueryResultIterator` to `EntitySetIterator`, but it did not yet provide
a public set value or set algebra.

The desired public abstraction needed to:

- avoid exposing an index's concrete storage type;
- preserve lazy execution and allow indexed results to borrow their backing
  sets without copying;
- support direct membership tests and reusable composition of query results;
- leave room for OR, NOT, predicates, and more targeted execution
  optimizations; and
- avoid materially slowing existing query, count, and sampling hot paths.

## Decision

Ixa introduced `EntitySet&lt;'a, E&gt;` as an opaque public set-expression type. Its
internal expression tree could contain source leaves and union, intersection,
and difference nodes. Construction applied simple algebraic reductions and
ordered operands using size and cost estimates; evaluation remained lazy.

`ContextEntitiesExt::query` returned an `EntitySet`. Callers could test
membership with `contains`, consume the set through `IntoIterator`, compose it
with other sets, or materialize entity IDs explicitly with `to_owned_vec`.
`with_query_results` passed an `EntitySet` to its callback instead of exposing
an `IndexSet`.

`EntitySetIterator` remained a distinct public streaming form.
`query_result_iterator` constructed it directly so common query and sampling
paths did not have to build an intermediate `EntitySet`. The iterator executed
the expression lazily and included specialized paths for source-only
intersections and other common cases.

The lowest-level `SourceSet` and `SourceSetIterator` types remained private.
They provided a uniform implementation boundary over population, index, and
property-derived sources without making those representations part of the
public API.

At adoption, an `EntitySet` could borrow an index set through the context.
Consequently its lifetime was tied to an immutable context borrow, so the
context could not be mutated while the set or its iterator remained live.
Set-algebra operations and iteration consumed the set, and neither
`EntitySet` nor `EntitySetIterator` was cloneable. Callers that needed owned
results across later context mutation could collect the entity IDs.

## Rationale

A public set abstraction expressed the semantics of a query result more
directly than an iterator alone. It allowed callers to combine independently
constructed results without forcing immediate execution, while keeping
membership tests and indexed traversal efficient.

Keeping representation and execution separate also created an optimization
boundary. Query construction could form and simplify a set expression, while
the iterator could select specialized execution strategies without changing
the query syntax or exposing index internals. More sophisticated expression
compilation, such as decision-diagram-based membership evaluation, remained a
possible later optimization rather than a requirement of the initial design.

The accompanying benchmarks showed that this abstraction could be introduced
without a broad performance penalty. Most measured cases stayed close to the
previous implementation, while some paths improved substantially, especially
`with_query_results` over multiple individually indexed properties. A few
counting and sampling cases regressed modestly; the design therefore retained
the direct iterator construction path and explicit hot-path specializations.

## Consequences

- Query results became first-class set values with lazy union, intersection,
  and difference.
- Public APIs no longer needed to expose the concrete `IndexSet` container.
- Indexed queries could continue to borrow their backing sets rather than copy
  entity IDs, and unindexed or composed queries did not need eager
  materialization.
- Query meaning, result representation, and execution strategy had clearer
  boundaries for future extension and optimization.
- `EntitySet`'s borrow prevented context mutation while a borrowed result was
  live. Callers needing to mutate the context had to end that borrow or collect
  an owned vector.
- Consuming operations and the lack of cloning made repeated use less
  convenient at adoption; callers could rerun a cheap query or materialize its
  results.
- Lazy expression execution added internal complexity and required specialized
  paths to protect performance-sensitive iteration, counting, and sampling.

## Alternatives considered

### Keep iterator-only query results

The existing iterator API was efficient for one-pass consumers, but it did not
provide a reusable value for membership tests or set algebra. Building OR,
NOT, and combinations of separate query results would have required additional
query-specific machinery or eager collection.

### Continue exposing `IndexSet` through scoped callbacks

This preserved a fast path for a single indexed result, but coupled the public
API to one index container. It also forced unindexed queries to materialize a
set before invoking the callback and did not naturally represent composed
results.

### Return an owned collection

An owned vector or set would permit context mutation and repeated traversal,
but every query would pay allocation and materialization costs. It would
discard the ability to represent an indexed result as a borrowed view and
would make lazy composition impossible.

### Compile set expressions immediately

More sophisticated compilation of membership and iteration logic was
considered a possible future optimization. Requiring it initially would have
added substantial complexity before the public abstraction and its performance
characteristics were established. The adopted expression tree preserved that
option without depending on it.

## References

- [ADR-0001: Adopt a typed entity and property architecture](0001-adopt-typed-entity-property-architecture.md)
- [ADR-0003: Maintain property indexes eagerly without `RefCell`](0003-maintain-property-indexes-eagerly.md)
- [Issue #746: `EntitySet` and `EntitySetIterator`](https://github.com/CDCgov/ixa/issues/746)
- [PR #745: Refactor query result types into the `entity_set` module](https://github.com/CDCgov/ixa/pull/745)
- [Commit `68b0f2c`: precursor module and type rename](https://github.com/CDCgov/ixa/commit/68b0f2cca26902ee38b887b0f860b99157c9d114)
- [PR #786: Introduce `EntitySet` and rewrite `EntitySetIterator`](https://github.com/CDCgov/ixa/pull/786)
- [Commit `a120994`: adopted implementation](https://github.com/CDCgov/ixa/commit/a120994ed9acd94765414d9482aa76a6069631e4)

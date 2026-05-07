# EntitySet

`EntitySet<E>` and `EntitySetIterator<E>` are the chosen public
representations for entity query results. They are not just implementation
details. Their purpose is to give callers a stable interface for working with
query results while hiding how those results are represented internally.

The query API itself is covered in the
[Entity System Design Notes](entities-design-notes.md) chapter. This chapter
focuses on the result types returned by that API.

## Role in Queries

The public query APIs accept either the entity marker type for a whole-population
query or a `with!(Entity, ...)` value for property filters. Query results are
then represented as one of two forms:

- `EntitySet<E>` when code needs a reusable set expression.
- `EntitySetIterator<E>` when code can stream matching entity IDs directly.

`Context::query` returns an `EntitySet<E>`. `Context::query_result_iterator`
returns an `EntitySetIterator<E>`. `Context::with_query_results` gives scoped
access to an `EntitySet<E>` through a callback.

Other APIs, such as `query_entity_count` and sampling methods, use these
representations as appropriate.

## EntitySet

`EntitySet<E>` represents a set expression over entity IDs. The current
implementation can represent:

- a source set, such as the whole population or an index bucket;
- intersections;
- unions;
- differences;
- the empty set.

This matters because an indexed query can often be represented by borrowing an
existing index bucket instead of constructing a new vector of entity IDs. For
unindexed queries, an `EntitySet` can represent the query as a composition of
sources and filters.

## Scoped Access and Borrowing

An `EntitySet<E>` may borrow from the `Context`. For example, an indexed query
can represent its result by holding an immutable reference to an index bucket.
That is the point of the abstraction: callers can work with query results
without knowing whether those results are backed by an index, a population
range, or a composed set expression.

The consequence is that the `Context` cannot be mutably borrowed while such an
`EntitySet` is live. This is ordinary Rust borrowing behavior, but it matters
for API design because many model operations mutate the context.

`Context::with_query_results` exists to provide scoped access to an
`EntitySet`. The callback receives the set, uses it, and then the borrow ends
when the callback returns. This makes the borrowing boundary explicit and keeps
later context mutation straightforward.

## EntitySetIterator

`EntitySetIterator<E>` is the streaming form. It yields `EntityId<E>` values and
has optimized paths for common cases such as whole-population iteration and
indexed source iteration.

Query code sometimes constructs an iterator directly instead of first building
an `EntitySet`. This is a performance choice: in tight loops, avoiding an
intermediate set expression can reduce overhead.

## Whole-Population and Empty Queries

Whole-population queries have special paths. Passing the entity marker type,
such as `Person`, means "all entities of this type." Internally, this can use a
`PopulationIterator<E>` over the entity IDs from `0..entity_count`.

Empty result sets also have explicit representations so query code can avoid
unnecessary iteration when a lookup proves there are no matching entities.

## Relationship to Indexes

Full indexes can provide the entity IDs for a property value directly, so they
can back both `EntitySet` and `EntitySetIterator` results.

Value-count indexes only store counts. They can speed up `query_entity_count`,
but they cannot provide entity IDs for `EntitySet` or `EntitySetIterator`.

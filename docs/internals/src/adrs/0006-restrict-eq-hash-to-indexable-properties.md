# ADR-0006: Restrict `Eq + Hash` to indexable properties

| Field | Value |
| --- | --- |
| Decision date | 2026-06-15 (merge of PR #937) |
| Recorded | 2026-07-29 |
| Status | Accepted |
| GitHub issue | [#909](https://github.com/CDCgov/ixa/issues/909) |
| Pull requests | [#927](https://github.com/CDCgov/ixa/pull/927), [#937](https://github.com/CDCgov/ixa/pull/937) |
| Feature branches | `RobertJacobsonCDC_remove_shared_indexes`; preparatory `RobertJacobsonCDC_909_indexable_property` |

## Summary

Ixa stopped requiring every property value to implement `Eq` and `Hash`.
`Property&lt;E&gt;` retained only the capabilities needed for storage, events,
derived computation, and linear query scans, while a new
`IndexableProperty&lt;E&gt;` marker expressed the additional `Eq + Hash` capability
required by hash-map-backed property indexes.

Index creation APIs required `IndexableProperty&lt;E&gt;`, but ordinary query APIs
continued to accept any `Property&lt;E&gt;` and fell back to scanning when no index
existed. This allowed unindexed properties containing values such as plain
`f32` or `f64` without forcing artificial equality and hashing semantics merely
to participate in the property system.

## Context

After [ADR-0005](0005-key-indexes-by-concrete-property-values.md), full and
value-count indexes were keyed directly by the concrete property value `P`:

```text
HashMap<P, IndexSet<EntityId<E>>>
HashMap<P, usize>
```

Those maps legitimately required `P: Eq + Hash`. The general property trait,
however, inherited from `AnyProperty`, which imposed
`Copy + Debug + PartialEq + Eq + Hash + 'static` on every property whether or
not an index was ever created.

Most property behavior did not need the stronger bounds. Column storage and
events copied values, derived-property evaluation returned them, and unindexed
queries compared them with `PartialEq`. Requiring `Eq + Hash` globally made the
property abstraction claim a capability that many callers never used.

Floating-point properties exposed the practical cost. Rust's `f32` and `f64`
implement `PartialEq` but not `Eq` or `Hash` because values such as NaN do not
have ordinary equivalence semantics. Before this decision, even an unindexed
floating-point property needed a wrapper or generated/manual equality and
hashing implementations.

The immediately preceding feature-branch changes had removed shared index
ownership and canonical key types. Those changes made the true capability
boundary visible: a concrete index required hash-compatible `P` values, while
the concrete property store itself did not.

## Decision

Ixa split general property requirements from index-key requirements.

### General properties

The `AnyProperty` trait was removed. `Property&lt;E&gt;` directly required:

```rust
Copy + Debug + PartialEq + 'static
```

These bounds applied to non-derived and derived properties regardless of
whether they could be indexed.

### Indexable properties

Ixa introduced the entity-specific marker trait:

```rust
pub trait IndexableProperty<E: Entity>:
    Property<E> + Eq + Hash {}
```

A blanket implementation made every `Property&lt;E&gt; + Eq + Hash` automatically
an `IndexableProperty&lt;E&gt;`. The trait remained generic over `E` because the same
Rust value type could implement `Property` differently for different entity
types.

`ContextEntitiesExt::index_property` and `index_property_counts`, property-store
index construction, and the concrete `FullIndex` and `ValueCountIndex`
implementations required `P: IndexableProperty&lt;E&gt;`. Attempting to index a
property without `Eq + Hash` therefore failed at compile time.

Normal add, get, set, event, derived-property, and query APIs remained bounded
by `P: Property&lt;E&gt;`. A non-indexable property could be queried through the
existing linear scan path.

Other features that used hash-map keys retained their own stronger bounds.
Value-change counters, for example, continued to require hash-compatible
property and stratum values; using `IndexableProperty` as shorthand for a
property's equivalent `Property + Eq + Hash` bound did not require that an
index actually be enabled.

### Optional typed index implementations

The property store needed to compile for every `P: Property&lt;E&gt;` without
requiring that an index implementation for `P` also compile. The earlier
`PropertyIndex&lt;E, P&gt;` enum embedded `FullIndex&lt;E, P&gt;` and
`ValueCountIndex&lt;E, P&gt;` variants directly, which propagated their key bounds
into the store's type.

Ixa replaced that enum with a typed `PropertyIndex&lt;E, P&gt;` interface and stored:

```rust
Option<Box<dyn PropertyIndex<E, P>>>
```

`None` represented an unindexed property. Concrete full and value-count index
types implemented the interface only when `P: IndexableProperty&lt;E&gt;`, and were
boxed only by APIs carrying that bound. Existing-index lookup and property
change paths could remain generic over `P: Property&lt;E&gt;`: they returned
unsupported or did nothing when the optional index was absent, and delegated
through the typed trait object when it was present.

The trait object erased which index implementation was installed, not the
property type. It deliberately remained `dyn PropertyIndex&lt;E, P&gt;` because index
operations consumed typed `&amp;P` values.

### Query-source identity without hashing

Property-backed `EntitySet` sources had temporarily used a hash of the logical
query value to recognize equivalent sources. That would have preserved a hidden
`Hash` requirement on every property.

Ixa removed the hash-based source ID. A type-erased property source instead
exposed its logical representative property ID and query parts. Equality
reconstructed the typed property value through
`Property::value_from_query_parts` and compared it with `PartialEq`. This
preserved equivalent multi-property routing without requiring unindexed query
values to be hashable.

### Macro behavior

The property-definition macros continued to derive or generate `Eq` and `Hash`
by default so common properties remained indexable without extra boilerplate.
Callers could use `impl_eq_hash = neither` for non-indexable types.

PR #927 had already added standalone `impl_property_eq!`,
`impl_property_hash!`, and `impl_property_eq_hash!` helpers for manually
declared types that needed Ixa's generated semantics. This preparation made the
trait split usable without changing the decision that equality and hashing were
optional for ordinary properties.

## Rationale

Trait bounds should describe capabilities actually required at an API
boundary. `PartialEq` was fundamental to property queries, but `Eq + Hash`
belonged to hash-map indexing. Moving the stronger requirement to index
construction made invalid combinations fail statically without burdening
unrelated property behavior.

The marker trait gave public and internal APIs one clear name for the
capability while retaining ordinary Rust blanket-implementation behavior. It
did not introduce methods or a second property model.

The optional typed trait object contained the stronger bound at construction
time. It avoided making all `PropertyValueStoreCore&lt;E, P&gt;` instantiations
indexable and preserved typed `P` operations after dynamic dispatch. This was a
more focused erasure boundary than erasing the property type itself.

Replacing hash-based logical-source identity with value reconstruction also
matched the semantics more directly. Source equality compared logical query
values instead of treating a hash as identity, and it relied only on the
`PartialEq` capability every property already needed.

## Consequences

- Unindexed properties could contain plain floating-point values or other
  `PartialEq` types without implementing `Eq` or `Hash`.
- Indexability became a compile-time capability expressed at index creation.
- Unindexed query semantics and linear scanning remained available for all
  properties.
- `AnyProperty` disappeared, and the common property requirements became
  visible directly on `Property&lt;E&gt;`.
- Enabled indexes required a heap allocation and dynamic dispatch through a
  typed trait object. Unindexed stores carried an empty optional pointer rather
  than an enum with concrete index variants.
- Property-backed source comparison reconstructed and compared logical values
  instead of comparing precomputed hashes.
- Most macro-defined properties remained indexable by default. Authors had to
  opt out when `Eq + Hash` was unwanted or invalid, so the public convenience
  default was broader than the minimum `Property&lt;E&gt;` contract.
- Indexable floating-point properties still required an explicit semantic
  choice: generated bitwise behavior, a manual implementation, or a wrapper
  type such as `OrderedFloat`.
- Hash-based features other than indexing retained appropriate local
  `Eq + Hash` bounds.

This was the third decision in the
`RobertJacobsonCDC_remove_shared_indexes` branch sequence. It depended on
[ADR-0004](0004-give-properties-concrete-index-ownership.md) aligning indexes
with concrete property stores and
[ADR-0005](0005-key-indexes-by-concrete-property-values.md) making `P` the
concrete index key. All three decisions merged together in PR #937.

## Alternatives considered

### Keep `Eq + Hash` on every property

This preserved a simpler uniform bound and avoided boxing index
implementations. It was rejected because it imposed hash-key semantics on
properties that were only stored, compared, or scanned and made ordinary
floating-point properties unnecessarily difficult to define.

### Reintroduce a separate canonical index-key type

A hashable associated key type could have allowed non-hashable property values
to be indexed through a transformed representation.
[ADR-0005](0005-key-indexes-by-concrete-property-values.md) had removed that
API because its conversion and macro complexity had no demonstrated use beyond
multi-property normalization. Reintroducing it would solve a different problem
than allowing genuinely unindexed properties to have weaker bounds.

### Require `Eq + Hash` on the property store but not the public trait

Moving the bounds off `Property&lt;E&gt;` without changing
`PropertyValueStoreCore&lt;E, P&gt;` would not work: every registered property creates
a store, including properties that can never satisfy the index variants'
bounds. The optional boxed interface was chosen to keep the store itself
available for any `Property&lt;E&gt;`.

### Erase the property type from indexes

A fully type-erased `dyn PropertyIndex&lt;E&gt;` could hide more bounds, but insertion,
removal, and lookup operate on concrete `&amp;P` values. Keeping `P` in
`dyn PropertyIndex&lt;E, P&gt;` retained static value typing and erased only the
choice between full and value-count index implementations.

### Keep hash-based property-source identity

This would have left an indirect `Hash` requirement on unindexed query values
or required a separate optional hashing mechanism. Comparing reconstructed
logical values used the already-required `PartialEq`, preserved multi-property
equivalence, and avoided treating hash collisions as equality.

## References

- [Issue #909](https://github.com/CDCgov/ixa/issues/909)
- [PR #927: Add user-facing property equality and hashing helpers](https://github.com/CDCgov/ixa/pull/927)
- [Commit `45b595d`: preparatory equality/hash macro extraction](https://github.com/CDCgov/ixa/commit/45b595de72fe6e6c9362980509cfc4e9914e4085)
- [PR #937: Introduce `IndexableProperty`](https://github.com/CDCgov/ixa/pull/937)
- [Feature-branch commit `4e914d8`](https://github.com/CDCgov/ixa/commit/4e914d8f32ea8adef979c8581a6a8a4a1fc0a325)
- [Merged commit `7b59c16`](https://github.com/CDCgov/ixa/commit/7b59c16bbc8ffdbb0e3e63186652ffa3bace6f3d)

The tree at the feature-branch tip `4e914d8` matches the merged tree at
`7b59c16`. The branch commit isolates this trait-bound decision from the two
preceding changes that were squashed into the same `main` commit.

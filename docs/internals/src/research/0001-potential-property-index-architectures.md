# Research-0001: Potential property index architectures

**Investigation date:** 2026-05-07 (best estimate from the source artifact)

> This is a speculative historical design exploration. It does not describe an
> adopted architecture or the current property-index implementation.

## Background and motivation

Right now there are only two: a `FullIndex` that stores a set of entity IDs for each property value (that an entity has), and a `ValueCountIndex`, which only stores the _count_ of entity IDs having each property.

Our design work is motivated by the observation that many useful capabilities can be implemented as a kind of index on a property:

- We might hypothetically want a `BPlusTreeIndex` index that speeds up queries involving inequality, e.g. "find all people under the age of 50."
- The `ValueCountIndex` itself is a specific kind of aggregator, one that counts how many entities have each value for a property. We can imagine other kinds of aggregation.
- We have a whole `Network` module implementing a directed graph system that can be used, for example, to implement contact networks. But this same functionality could be replaced with a particular kind of index. Imagine we have a `Person` entity and a ZST naming the type of network that we'll generically call `Network`. Then we can define another entity `Edges&lt;Person, NetworkType&gt;` with a property `ToPerson`. If we implement a "dense" index in which the keys are `EdgeID` values that are used to just index into a vector and the values are stored in a vector, say, then a "Network" is just an `Edges&lt;Person, NetworkType&gt;` with a property `ToPerson` having just such an index. We'd just identify each `EdgeId` with the `PersonId` the edge originates _from_ and the value of `ToPerson` as the `PersonId` the edge terminates at. (This only models 1-to-many relationships but could easily be modified to many-to-many relationships.)
- Suppose we want to count incidences. To be concrete, suppose we want to count how many hospitalizations we have each month. We might want a more generic name for the type of the index, like `ValueChangeCounter` or something. This index is very similar to `ValueCountIndex` in that it stores counts for each property value; it differs from `ValueCountIndex` in that it never _decrements_ its counts. If, during the month, someone enters the hospital and then leaves the hospital, it's still counted as a hospitalization. Since we want hospitalizations each month in this hypothetical scenario, we would have a periodic plan that fires each month to report the data (to a CSV file, say) and then _clear_ the counts, resetting everything to zero.
- Now suppose we want to count incidence of hospitalizations, but we want it _stratified_ by `AgeGroup` and `VaccinationStatus`. That is, for values of the tripple `(AgeGroup, VaccinationStatus, Hospitalized)` , we want to increment a counter associated to the tripple _whenever `Hospitalized` changes_. Notice we only count when the last coordinate changes for a `PersonId`. We might implement this as a special kind of index on the multi-property `(AgeGroup, VaccinationStatus, Hospitalized)`.

The last example of incidence tracking is especially interesting, because our current model of indexing for multi-properties intentionally disregards the ordering of the component properties, whereas in our hypothetical example the _last_ coordinate property is distinguished. (Said another way, we might have to keep track of the distinguished component property using a different mechanism.) Also, for incidence tracking, we would want to be able to have _multiple_ `ValueChangeCounter` indexes so that we can have separate daily, weekly, and monthly hospitalization counts, for example.

So we want the ability to have multiple different kinds of indexes for each property, and potentially multiple of the same kind of index (as in our hypothetical `ValueChangeCounter`). But we also have other constraints:

- It makes no sense to have _both_ a `FullIndex` and a `ValueCountIndex`. If you already have a `FullIndex`, you don't need a `ValueCountIndex`, because the capabilities of `FullIndex` completely subsume the capabilities of `ValueCountIndex`
- Likewise, it probably makes little sense to have both a `BPlusTreeIndex` and a `FullIndex`.

What makes something an "index":

- It needs to be maintained on property value changes. This is done in two phases:
  - Removal of an entity ID for an old value (in `PropertyValueStore::create_partial_property_change`)
  - Addition of an entity ID for a new value, with the old value also available at this time (in `PartialPropertyChangeEvent::emit_in_context`)
- It supports some kind of querying

## Possible Implementation Mechanisms

## Add more fields to `PropertyValueStoreCore&lt;E, P&gt;`

The `PropertyValueStoreCore&lt;E, P&gt;` type started out as

```rust
pub struct PropertyValueStoreCore<E: Entity, P: Property<E>> {
    /// The backing storage vector for the property. Always empty if the property is derived.
    pub(super) data: RawPropertyValueVec<P>,
    /// An index mapping `property_value` to `set_of_entities`.
    pub(crate) index: Option<Index<E, P>>
}
```

It will soon have another `incidence_counts` field holding aggregator indexes. We will have separate logic for the `index` and `incidence_counts` fields and custom logic for when we add a `ValueCountIndex` or `FullIndex` that takes care of existing constraints. We could just continue this pattern of multiplying the number of fields on `PropertyValueStoreCore`:

```rust
pub struct PropertyValueStoreCore<E: Entity, P: Property<E>> {
    /// The backing storage vector for the property. Always empty if the property is derived.
    pub(super) data: RawPropertyValueVec<P>,
    /// An index mapping `property_value` to `set_of_entities`.
    pub(crate) index: PropertyIndex<E, P>,
    /// Incidence tracking: records property value transitions
    pub(crate) incidence_counts: Vec<RefCell<Box<dyn ValueChangeCountIndex<E, P>>>>,
  	/// B+ Tree-based index for fast queries on inequalities
  	pub(crate) bpt_index: Option<BPTreeIndex<E, P>>,
  	/// "Dense" index for things like adjacency lists on graphs with many edges
  	pub(crate) dense_index: Option<DenseIndex<E, P>>,
  	/// "Semi-dense" index for cases of a small number of entities per value 
  	/// Entities are stored in a vector.
  	pub(crate) semi_dense_index: Option<SemiDenseIndex<E, P>>,
  	// ...
}
```

- The constraints about which queries can co-occur with which other queries would be explicit but potentially complicated.
- The logic about which index to consult for which queries would be explicit but potentially complicated.
- Public API would have explicit routes to explicit fields on `PropertyValueStoreCore`
- The update logic (currently in `PropertyValueStore::create_partial_property_change` and `PartialPropertyChangeEvent::emit_in_context`) would need to grow with each new field, check existence for each and every index type.

## Design a generic API for the `PropertyIndex&lt;E, P&gt;` trait

We could simplify `PropertyValueStoreCore` to

```rust
pub struct PropertyValueStoreCore<E: Entity, P: Property<E>> {
    /// The backing storage vector for the property. Always empty if the property is derived.
    pub(super) data: RawPropertyValueVec<P>,
    /// An index mapping `property_value` to `set_of_entities`.
    pub(crate) indexes: Vec<PropertyIndex<E, P>>
}
```

Then we carefully design an API on `PropertyIndex&lt;E, P&gt;` the implementations of which encode constraints about co-existence and query choices. Update logic would just loop over `indexes` calling relevant update methods.

## Capability System

Putting all indexes in an `indexes` field on `PropertyValueStoreCore` may not be enough to express different index capabilities, or a preference for one index over another if multiple indexes have a capability. This alternative enriches the previous idea with a capability system in which `PropertyIndex` instances can be queried for their capabilities, and `PropertyValueStoreCore` maintains a table of which index to use for which capability.

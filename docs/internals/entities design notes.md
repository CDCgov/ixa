# Notes about `Entity` Implementation

## Querying entities

Q: Should `Context::with_query_results` be generic over the type returned by the closure? That way the closures can return a computed value. Without it, they'd need something like:

```rust
let mut computed_value: usize = 0;
context.with_query_results(
    (Age(30), Height(100)),
    &mut |people_set| {
      // Mutate the captured value.
      computed_value = people_set.len();
    },
);
```

### Two ways to get an iterator over all `EntityId<E>`s

There are two ways to iterate over all `EntityId<E>`s:

```rust
// Call `query_entities` with an empty query
let result_iterator: QueryResultIterator<Person> = context.query_entities::<Person>(());
// Ask for an `EntityIterator<E>` directly
let person_iterator: EntityIterator<Person> = context.get_population_iterator::<E>();
```

The difference is their types. Both implement the trait `Iterator< Item=EntityId<E> >`. The ` QueryResultIterator<Person>` type wraps an (unboxed) `EntityIterator<Person>` internally and is a few bytes larger. This might suggest `context.get_population_iterator::<E>()` should be preferred for this use case, but it probably doesn't matter in practice.

### Methods on `Context` that just call methods on `Query`

#### Two ways to execute a query

```rust
let results = context.query_result_iterator((Age(40), RiskCategory::High));
// Internally just calls method on `Query<E>`:
let results = (Age(40), RiskCategory::High).new_query_result_iterator(context);
```

This is probably fine, as client code interacts with ixa almost exclusively through the `Context`.

#### Two ways to sample an entity / entities

```rust
// Suppose we already have a `QueryResultIterator<E>`, say, from this call.
let my_query_result_iterator = context.query_result_iterator((Age(40), RiskCategory::High));

// We can sample from it directly. Requires the RNG.
let results = my_query_result_iterator.sample_entity(rng);

// Or we can pass the query to the sample function.
let results = context.sample_entity(MyRng, (Age(40), RiskCategory::High));
```

We only give client code access to the RNG itself in `Context::sample`, so maybe we make the method on the `QueryResultIterator` private (`pub(crate)`)? The method on `Context` is:

```rust
    pub fn sample_entity<R, E, Q>(&self, rng_id: R, query: Q) -> Option<EntityId<E>> {
        let query_result = self.query_result_iterator(query);
        self.sample(rng_id, move |rng| query_result.sample_entity(rng))
    }
```



### No `Query`/`PropertyList` implementation for naked `Property<E>`

Blanket implementations of `PropertyList`/`Query` for `Property<E>`cause a conflicting implementation error for the `()` type:

```
error[E0119]: conflicting implementations of trait `entity::query::Query<_>` for type `()`
  --> src/entity/query/query_impls.rs:39:1
   |
13 | impl<E: Entity> Query<E> for () {
   | ------------------------------- first implementation here
...
39 | impl<E: Entity, P1: Property<E>> Query<E> for P1 {
   | ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ conflicting implementation for `()`
   |
   = note: downstream crates may implement trait `entity::property::Property<_>` for type `()`
```

The reason is that we want both `PropertyList` and `Query` traits implemented on the "empty query"/"empty property list" for user convenience. An empty query returns every entity, while an empty property list sets all properties to their default values for the new entity.

The problem is, we _also_ want to use a single property in place of an ugly singeton tuple:

```rust
// Loads of parens, with a mysteriously required comma.
context.add_entity((Age(9),));
// Compared to this beauty:
context.add_entity(Age(9));
```

To acheive this, we provide a blanket implementation of both  `PropertyList`/`Query`  for properties.

The problem is, Rust does not allow this, because downstream client code COULD implement `Property<E>` on `()`, and if they did so, there would be to conflicting implementations.

Normally client code wouldn't be allowed to implement a foreign trait `Property` on a foreign type `()`, but since `Property<E>` has a generic parameter, it's possible to fill that generic with a local type and thus make `Property<E>` local.

## Multi-Properties

Suppose we have

```rust
define_multi_property!((Age, Status));
// And then later, possibly in a different module
define_multi_property!((Status, Age));
```

**Q:** Suppose `Age` changes. Do we emit `PropertyChangeEvent`s for BOTH `(Age, Status)` AND `(Status, Age)`?

```rust
context.subscribe_to_event::<PropertyChangeEvent<(Age, Status)>>(my_event_handler);
context.subscribe_to_event::<PropertyChangeEvent<(Status, age)>>(another_event_handler);
```

How do we _share_ indexes but emit events separately?

**Answer:** Rename `Property::index()` to `Property::id()`. Every property has its own `Property::id()` initialized in its `ctor` (as originally imagined). Then implement `Property::index_id()`, which returns the `id` of the `PropertyStore` managing the value index on behalf of the property.

**Q:** Should multi-properties be indexed automatically by default? Is there a use-case for multi-properties outside of jointly indexing its components? Right now we require:

```rust
define_multi_property!((Age, Status));
// And then later...
context.index_property::<Person, (Age, Status)>();
```

This is not ergonomic at best.

**A:**  Use cases:

- monitoring `PropertyChangeEvent`s — this is actually what motivated them if I recall correctly.
- counting value combinations (a different kind of index).



To auto-enable indexing for multi-properties, I think we'd add the method `Property<E>::index_by_default()`. This would default to `false` but would be overridden for multi-properties (and potentially future esoteric property types). Client code would then have to explicitely override this by calling `context.set_indexed::<Person, (Age, InfectionStatus)>(false);`. That's certainly awkward, but it's



## Updating dependents during `set_property`

When a non-derived property is updated, all (derived) properties that depend on it need to be updated:

1. Their index needs to be updated
2. A property change event needs to be emitted

The property being set is completely known, while the dependents are completely type erased (represented by their index).

Several things make this process annoying:

- ~~The entity type is known unambiguously, but we store properties with the entity type erased, which makes passing the `EntityId<Entity>` to the dependent annoying.~~ UPDATE: We decided to include the entity type `E` in the `PropertyStore`.
- The both the old value and the new value of the dependent need to be computed, which involves looking up the values of their dependencies, which requires (immutable) access to not just the `PropertyStore` but the `Context` (in order to fetch global properties as well).
    - ...which means we need immutable access to some parts of the `PropertyStore` (to look up dependencies) while we mutate other parts of the `PropertyStore` (to update the index)
    - ...which means we need to do potentially a lot of potentially redundant property lookups of values

Mechanism to update



## `smallbox` Optimization Opportunities

The allocation within `Context::set_property` and related methods is really unfortunate. The [`smallbox`](https://crates.io/crates/smallbox) is like `Box` but stores small enough values inline. (Similarly for the `smallvec` crate.) We should check if it's worth doing.

Issue [#624](https://github.com/CDCgov/ixa/issues/624).

## Initialization / Registration of Properties

We still have a problem with initialization / registration of properties.

To store properties, we use a `Vec<OnceCell<Box<dyn Any>>>`; the `OnceCell` is used because we don't want to bother initializing anything for properties that are never used. Only when client code attempts to actually _use_ a property is the `OnceCell` initialized.

Except what if the property is derived? It might never be used by client code, but if its dependencies change, internally ixa needs to emit a change event for the property, which means (at minimum) computing the value of the property.

- Our current implementation calls `register_property` _everywhere_ we need the property to exist. This effectively initializes all _dependents_ of a property whenever a property is initialized.

What does it mean to "register" / "initialize" a property?

1. Creation of a `StoredPeopleProperties` object (but *not* allocation of backing vector)
2. Determination of `is_required` (by virtue of (1))
3. Determination of dependencies and dependents
4. Creation of `Index` object (but not allocation of storage for nor computation of index)
5. Determination of `is_indexed` (by virtue of (4))

Observations:

- Indexing is always deferred to the first query.
- Allocation of storage is always done on an as-needed basis.
- A property must be _explicitly_ set as indexed, so (5) only actually sets the default `is_indexed` to `false`. It doesn't actually do anything. In fact, (4) is just a convenience.
- (1) and (4) are partly about instantiating a mechanism that can perform functions on behalf of the (typed) property in a type-erased way.

Let's tease out registration/initialization that is independent of a particular `Context` instance vs. registration / initialization that is `Context`-specific.

Solution so far:

- We collect metadata via ctors before `main()`, computing
    - Property Dependents (NOT dependencies, which are known statically)
    - List of properties associated with each entity
    - List of _required_ properties associated with each entity
    - An `index` for each property which we use for fast lookup.
    - Storing a constructor (function pointer) that knows how to construct a `Box<PropertyValueStoreCore<E, P>>` type-erased as a `Box<dyn Any>`.

My strategy has been to put implementation that depends on knowledge of types in `Box<dyn PropertyValueStore<E>>`. In `set_value` I can then look up the relevant `PropertyValueStore<E>` and call the method. So far this has made a lot of sense, because the implementation generally requires access to data stored in the underlying `PropertyValueStoreCore<E, P>`. But if it's implementation that doesn't depend on the data in the `PropertyValueStoreCore<E, P>`, maybe the function pointer should live somewhere else?

## Registry pattern

Should we replace all uses of `TypeId` with `RegisteredItem::id()`?

- `entity::entity_store::ENTITY_METADATA` maps `TypeId` to the typle `(Vec<TypeId>, Vec<TypeId>)`: the entity `TypeId` is mapped to lists of `Property<E>` `TypeId`s.

  This makes sense, because the use case is checking that a `PropertyList` contains all required properties, so we're checking equality of types.

- `entity::property_store::PROPERTY_METADATA` maps `usize` to `Vec<usize>`: the `Property::id()` is mapped to a vector containing the  `Property::id()`'s of all of that property's dependents.

  This makes sense, because the use case is looking up the properties in the property store, and they are addressable via the id.

- Each `Entity` type now has its own series of property IDs. Thus, it's possible for `Property<E1>::id() == Property<E2>::id()` if `E1!=E2`. Using a `TypeID` therefore provides a little extra type safety. (Using the pair `(E::id(), P::id())` is an alternative—we do this for `ixa::entity::property_store::PROPERTY_METADATA`, for example.)

## Integration with `Context`

### API on `Context`

**Q:** Should `add_entity<E: Entity>` return a `Result<EntityId<E>, IxaError>`? Right now it fails only if `InitializationList` is invalid.

```rust
pub fn add_entity<E: Entity, PL: PropertyList<E>>(&mut self, property_list: PL) -> Result<EntityId<E>, IxaError>;
```

- Makes sense to return a `Result` if we let user add an entity, say, at debug console or via web API.
- Makes sense to just panic if we only care about calling the method from client code directly.
- I've found it's easy to just try to add a person, throw away the result, and assume the add was successful, even though there was an error. So maybe we should panic. We could have a `try_add_entity` for fallible situations.

**Q:** Should we expose `Context::is_property_indexed::<E, P>()` in the public API? (Returns `true` if the property is being indexed.) Right now the only use case is in unit tests.

- It's possible for  `Context::is_property_indexed::<E, P>()` to return `true` even if `context.index_property::<E, P>()` was never called, because `P` might be "equivalent" to some other property `Q` that is indexed. Right now this only happens for multi-properties, e.g. `(Age, Height)` is equivalent to `(Height, Age)`. But presumably client code knows which properties its indexing.
- It's just a noop—not an error—to call `context.index_property::<E, P>()` multiple times, so client code doesn't need to check before indexing.
- It's not exactly adding complexity, especially since we use it for tests anyway.
- If we make it public API, should it be part of the `PluginContext`?

**Q:** What methods on `Context` should be a part of the  `PluginContext` trait? Since  `PluginContext` defines the basic API available to data plugins, do we want data plugins to have access to the Entity subsystem?



### Should `EntityStore` / `PropertyStore` be fields of `Context`?

Edit: It's easier to have `PropertyStore<E>` know the entity type `E`, so we store the `PropertyStore<E>` (in a type-erased way) in `EntityStore`

Arguments for making the `EntityStore` ~~and `PropertyStore`~~ a field of `Context`:

- Not every subsystem cares about entities/properties, but a _lot_ of them do.
  (`PluginContext` defines the minimal interface all `DataPlugin`s can assume exists. Half
  of the trait extension constraints in `PluginContext` are related to `PeopleContext`.)
- Accesses to these stores are often in the hottest paths / tightest loops,
  which recommends minimizing indirection (one fewer pointer dereference).

Arguments against making the `EntityStore` and `PropertyStore` fields of `Context`:

- Historically the only intrinsic properties and functions of the concrete `Context` type is
  managing the timeline (events and plans). All other functionality is (was) provided by plugins.
  Adding entities (`EntityStore` ~~and `PropertyStore`~~) to `Context` expands its responsibilities
  from "events that happen in time" to also include "state of the world". Philosophically, one
  could argue this violates the "separation of concerns" / "single responsibility" principle.
  (Counterargument: A good software engineer is not constrained by rules of thumb.)
- Is the indirection required for accessing a `DataPlugin` even measurable? I've attempted to benchmark this, but it's difficult to measure. The overhead is somewhere between 0%-10% in the tightest loops, but obviously [Amdahl's Law](https://en.wikipedia.org/wiki/Amdahl%27s_law) applies.

## Types

### `struct EntityId<E: Entity>(usize, PhantomData<E>)`

- The entity ID should know the `Entity` type so that a `PersonId` can't be used as a `SettingId`.
- The original `PersonId` type is opaque–it cannot be created or destructured outside of the `ixa` crate.  To achieve the same thing, we do this: `EntityId<E: Entity>(usize, PhantomData<E>)`
- `Entity` type itself does not store the entity count, because we don't want client code to be able to create a new entity (or modify the entity count), and the `Entity` types are implemented in client code. So we store the entity count in the `EntityStore` (or `EntityRecord`.

## Properties

### Visibility of fields

Right now I just make all fields of properties defined with `define_property` (and related macros) `pub` to simplify parsing. But we could instead let the user specify visibility of fields.

Related: Default visibility of synthesized types. Right now I make `Entity` and `Property<Entity>` types `pub`. We could let the user specify.

### What should the default display implementation be for synthesized property newtypes?

Right now, something like `define_property!(struct Age(u8), Person, is_required=true)` generates a default string implementation using `format!("{:?}", self)`. But we special-case new types that just wrap an `Option<T>`:

```rust
define_property!(struct Vaccine(Option<u32>), Person, default_const=None);
let no_vaccine = Vaccine(None);
no_vaccine.get_display(); // "None"
let some_vaccine = Vaccine(Some(11));
some_vaccine.get_display(); // "Vaccine(11)"
```

An alternative is to unwrap every type for display:

```rust
define_property!(struct Age(u8), Person, is_required=true);
let age = Age(14);
age.get_display(); // "14"
```

The wrapped `Option<T>` case becomes

```rust
define_property!(struct Vaccine(Option<u32>), Person, default_const=None);
let no_vaccine = Vaccine(None);
no_vaccine.get_display(); // "None"
let some_vaccine = Vaccine(Some(11));
some_vaccine.get_display(); // "11"
```

The downside to this scheme is that `Age(11)` prints the same as `Vaccine(Some(11))`.

As currently implemented, client code can supply their own `display_impl`:

```rust
define_property!(
  struct Height(u8),
  Person,
  is_required=true,
  display_impl = |val: &Height| {
    let feet = val.0 / 12;
    let inches = val.0 - 12*feet;
    format!("{}' {}\"", feet, inches)
  }
);
```



### Required versus Explicit

Originally it appears we assumed that it is not possible to have a non-required property without a default value. (I think we just let an internal `unwrap` fail.) Should we enforce this in the macro implementations? (Related to the next section as well.)

**Answer:** Yes, as per discussion. New implementation requires a property either have a default value or is required. Enforced at compile time with macros.

Rationale: https://app.excalidraw.com/s/7Clp38IaTj4/6og9r5Urrhu

### Semantics of Property Change

- If you try to set a property to the value it already has, is a property change event emitted? (Previously, yes.)
- If you try to set an unset property to its default value, is a property change event emitted? (Previously, yes.)

I think we emit a property change even unconditionally, *because it was hard to keep track of whether derived properties actually changed value* due to type erasure.  And since we emit an event for derived properties, we also do so for non-derived properties.

EDIT: There doesn't seem to be strong opinions about this either way, but there's consensus that it's not much of a burden on client code to do a check. Because a change event already isn't reporting on the instantaneous state of the world but rather a report of a particular transaction, it's already incumbant on client code to check whether the event is actionable.

See `PropertyValueStore::replace()`.

Also: Previously, it appears we assumed that it is not possible to have a non-required property without a default
value. (EDIT: Sort of. We panic in this case.)

## General Stuff

### Internal checks

There are several places in `ixa` internals where we have checks for
things that should never happen unless there's a bug _in ixa itself_. These
checks could be conditionally removed depending on the build profile:

```rust
#[inline(always)]
fn expect_debug_only<T>(opt: Option<T>, msg: &str) -> T {
    if cfg!(debug_assertions) {
        opt.expect(msg)
    } else {
        unsafe { opt.unwrap_unchecked() }
    }
}

// or the plain vanilla inline version:
let value = if cfg!(debug_assertions) {
  property_store
          .get(entity_id)
          .expect("getting a property value with \"constant\" initialization should never fail")
} else {
  unsafe { property_store.get(entity_id).unwrap_unchecked() }
};
```

The actual performance impact of this is probably negligible. It
might be worth running some experiments to see if it's worth it.

### `define_rng!` macro

We have this "`rng_name_duplication_guard_`" thing that I don't understand. Some research shows it was a pre-Rust 2018 hack to prevent global singletons with the same name.

- It doesn't work anymore (maybe?).
- Why not have RNGs with the same name?
- The macro makes the RNG private, so it's not accessible outside of the module anyway.

Alternatives (that I am also suspicious of):

1. Just remove it. We trust client code to know how to refer to the correct symbol in other places.
2. "private trait" trick:

```rust
$crate::paste::paste! {
    trait [<__rng_guard_ $random_id>] {}
    impl [<__rng_guard_ $random_id>] for () {}
}
```

3. unique `const` generic:

```rust
struct __RngNameDupGuard<const NAME: &'static str>;
type _ = __RngNameDupGuard<{ stringify!($random_id) }>;
```

### Visibility of Synthesized types

We synthesize these types with macros _without_ a `pub` visibility (or any other visibility).

- `DataPlugin`
- RNGs
- `Entity`
- `Property<E: Entity>`

### Floating point property values

Floating point property values are annoying, because the primitive `f64` value does not implement `Hash` or `Eq`. We get around this by using the hash of the serialization of the value, `hash_serialized_128`, and we compare the 128 bit hash when we need equality.

But for most use cases we don't need—or even want—to use an `f64`. Instead, we want a finite real number. For these cases we should be using something like the [`decorum`](https://crates.io/crates/decorum) crate, which offers the [Real](https://docs.rs/decorum/0.4.0/decorum/type.Real.html) type and [ExtendedReal](https://docs.rs/decorum/0.4.0/decorum/type.ExtendedReal.html) type.

I still think we should use `hash_serialized_128`, because it gives us the power to compute the same hash for an array of serialized values and a tuple of those same values, which allows us to query multi-properties dynamically (potentially from the debug console or web API). The performance penalty seems to be not very high.

## Some things I don't love

This stuff is _probably fine_, and where it isn't fine, it's probably fixable over time. But if we can avoid any obvious tech debt now, we should.

- Sometimes we pass around a `Context` in places where we really just want a `PropertyStore` or at most an `EntityStore`. (Related to how deep/shallow various internal API are in the ownership model, the next bullet.)
- It's not always clear where some implementation lives. One philosophy is, all implementation that accesses a struct's data lives on the struct. But some structs are naturally _just_ data, and some imlementation is cross-cutting. (Purists would argue for a refactor for such cases.)
- Queries consisting of a single property need to use the singleton tuple notation: `(Age(30),)`. Gross. We could have a special method call specifically for the single property case purely for aesthetic reasons that just delegates to the standard method.

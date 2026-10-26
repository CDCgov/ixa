# Population Persistence

Ixa can separate population generation in order to reuse a constructed population
for multiple model runs by saving all entity counts and non-derived property values to a binary artifact:

```rust
context.save_population("population.bin")?;

let context = Context::from_population("population.bin")?;
```

`save_population` includes every registered entity type and every non-derived
property not declared with `persist = false`, including the sparse backing representation of constant-default
properties. `from_population` creates a fresh `Context`, validates that the
artifact has exactly the current entity and property schema, and restores those
counts and values without emitting entity or property events.

Population persistence is not simulation checkpointing. It does not save indexes,
event handlers, subscriptions, counters, global properties, simulation time,
scheduled plans, callbacks, RNG state, networks, data plugins, or profiling state.
Configure that state after loading. Indexes can be installed with the normal
indexing APIs and will be built from the restored values.

## Compatibility and trust

A population artifact may be imported only by the same build artifact that
exported it. Importing it with another build has unspecified behavior. Fully
qualified Rust type names are stored for routing and diagnostics, but they are not
a schema-versioning mechanism. Saving fails if two types share a type name, as
same-named items declared in sibling `const _: () = { .. };` blocks do.

Population artifacts are trusted application outputs, not a hardened interchange
format. Persisted property values must support `serde::Serialize` and owned
`serde::Deserialize`, and their Serde representation must be compatible with
`bincode-next`. In particular, representations that require a self-describing
Serde decoder are not supported.

## Skipping properties

`save_population` returns an error listing every non-derived property without
Serde, so a forgotten derive is not silently dropped. To skip a property on
purpose, declare it with `persist = false`:

```rust
#[derive(Copy, Clone, Debug, PartialEq)]
struct Nickname(&'static str);
impl_property!(Nickname, Person, default_const = Nickname(""), persist = false);
```

`define_property!` accepts `persist = false` too, but it always derives Serde, so
a property whose field type has no Serde must be declared with `impl_property!`.

After loading, a skipped property holds its `default_const` for every entity.
A required property (no `default_const`) cannot be skipped: saving fails if its
entity type has any entities. Entity types with no entities are not checked.

Saving streams binary data to a temporary file beside the destination and replaces
an existing destination only after encoding and flushing succeed. Loading likewise
streams property vectors from disk; it does not construct a second encoded copy of
the population in memory.

See `examples/population-persistence` for a complete example.

# Population Persistence

Ixa can export all entity populations in a [`Context`](https://ixa.rs/doc/ixa/struct.Context.html)
to a directory of CSV files and import them into another context. This is useful
when population generation is expensive or when several simulation runs should
start from the same entities and property values.

Population persistence includes:

- Every registered entity type, including entity types with no instances.
- Contiguous entity IDs, preserved exactly during import.
- Every non-derived property value, including values supplied by constant
  defaults.

Derived properties are recomputed from their imported dependencies. Networks,
global properties, random-number-generator state, plans, simulation time, data
plugins, event handlers, and indexes are not stored.

## Exporting and Importing

Import the extension trait and pass a directory path to
`export_population` or `import_population`:

```rust,ignore
use std::path::Path;

use ixa::{Context, ContextPopulationExt};

let source = Context::new();
// Build the source population.
source.export_population(Path::new("generated-population"))?;

let mut target = Context::new();
// Configure global properties, subscriptions, or empty indexes as needed.
target.import_population(Path::new("generated-population"))?;
```

The export destination must not already exist. Import requires every registered
entity population in the target context to be empty. Ixa reads and validates
the complete export before adding any entities to the target context.

Import preserves normal entity-creation behavior: it catches up existing
indexes and queues one `EntityCreatedEvent` for each imported entity. It does
not emit property-change events.

## Directory Format

An export directory contains:

```text
generated-population/
├── manifest.csv
├── entity_0000.csv
├── entity_0001.csv
└── ...
```

`manifest.csv` records the format version, fully qualified Rust entity type
names, and deterministic entity filenames. Each entity file is a wide CSV whose
first column is `entity_id`. Remaining columns use fully qualified Rust property
type names.

Entities, property columns, and rows have deterministic ordering. Import
requires an exact match between the exported entity/non-derived-property schema
and the current model. Renaming or moving an entity or property type changes its
fully qualified name and intentionally makes older exports incompatible.

## Supported Property Values

One property value must fit in one scalar CSV cell. Supported values are:

- Booleans, integers, floating-point values, and characters or nonempty string
  scalars.
- Unit-enum variants.
- Serde newtype wrappers around supported scalar values.
- `Option` values wrapping a supported scalar. An empty cell represents `None`.
- `EntityId` values, which are scalar IDs. Because the entire population is
  imported together, preserved IDs continue to refer to the same entities.

Empty strings are rejected because an empty cell is reserved for `None`.
Sequences, tuples, maps, multi-field structs, units, and data-carrying enum
variants are also rejected. Export returns an error naming the entity and
property instead of silently omitting unsupported data.

## Property Registration

`define_property!` automatically enables persistence when the generated type
implements `serde::Serialize` and owned `serde::Deserialize`. Types containing
borrowed data, such as `&'static str`, remain valid Ixa properties but cannot be
imported from owned CSV data; export reports them as non-persistable.

Properties declared manually with `impl_property!` must opt in explicitly:

```rust
use ixa::{define_entity, impl_property};

define_entity!(Person);

#[derive(
    Debug,
    PartialEq,
    Eq,
    Hash,
    Clone,
    Copy,
    serde::Serialize,
    serde::Deserialize,
)]
struct RiskScore(u16);

impl_property!(
    RiskScore,
    Person,
    default_const = RiskScore(0),
    persist = true
);
```

The `persist = true` option checks the Serde requirements at compile time.
Without it, a manual non-derived property remains usable normally, but a
whole-population export returns an error rather than producing an incomplete
snapshot.

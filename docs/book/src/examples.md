# Examples

The following examples are included in the [`examples/`](https://github.com/CDCgov/ixa/tree/main/examples) directory:

## Feature Demos

### `basic`

A minimal example that creates a `Context`, schedules a single plan, and prints
the current simulation time. Good starting point for understanding the basic
structure of an Ixa model.

```sh
cargo run --example basic
```

### `parameter-loading`

Demonstrates how to load simulation parameters from a JSON file using
`load_parameters_from_json` and store them as global properties.

```sh
cargo run --example parameter-loading
```

### `population-persistence`

Generates a small population, saves all entity counts and non-derived property
values, then loads the population into a fresh context and runs a toy model.

```sh
cargo run --example population-persistence
```

### `profiling`

Demonstrates the profiling module: counting events, opening spans, computing
statistics, and writing profiling data to JSON.

```sh
cargo run --example profiling --features profiling
```

## End-to-end Examples

### `basic-infection`

A simple SIR model with a constant force of infection applied to a
homogeneous population. Demonstrates entity definitions, property changes,
event observation, and report writing.

```sh
cargo run --example basic-infection
```

### `births-deaths`

Extends the basic infection model with birth and death processes, age groups,
and age-varying force of infection. Demonstrates dynamic population changes,
plan cancellation on death, and person property lookups.

```sh
cargo run --example births-deaths
```

### `network-hhmodel`

A network module (using Ixa's `network` extentension) which loads a population
with household structure from CSV files and spreads infection along network edges
with different transmission rates by edge type.

```sh
cargo run --example network-hhmodel
```

### `core-epi`

The smallest `core_epi` model: homogeneous mixing with every
`TransmissionManager` default, so the model only supplies an infectiousness
rate function and what happens on infection.

```sh
cargo run --example core-epi
```

### `core-epi-antiviral`

The `core-epi` model plus an antiviral intervention: a share of infected people
start treatment after a delay, which scales their infectiousness for the rest
of the infection. Runs the same seed with and without coverage and prints both
final sizes.

```sh
cargo run --example core-epi-antiviral
```

### `core-epi-settings`

Transmission through settings (household, school, workplace and community)
using `core_epi`'s settings module. Loads a line list from CSV, builds setting
groups from its ids, and splits each person's contacts across settings with an
itinerary.

```sh
cargo run --example core-epi-settings
```

### `core-epi-variants`

Two variants of one pathogen, carried as `core_epi`'s infection details: the
variant decides the rate function, how much a past infection protects and how
well a vaccine works. An escape variant is seeded after the first wave and
reinfects part of it.

```sh
cargo run --example core-epi-variants
```

## External examples

* [ixa-epi-covid](https://github.com/CDCgov/ixa-epi-isolation)
* [ixa-epi-isolation](https://github.com/CDCgov/ixa-epi-isolation)

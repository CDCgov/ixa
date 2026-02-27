# Examples

The following examples are included in the [`examples/`](https://github.com/CDCgov/ixa/tree/main/examples) directory:

## Feature Demos

### `basic`

A minimal example that creates a `Context`, schedules a single plan, and prints
the current simulation time. Good starting point for understanding the basic
structure of an ixa model.

```sh
cargo run --example basic
```

### `parameter-loading`

Demonstrates how to load simulation parameters from a JSON file using
`load_parameters_from_json` and store them as global properties.

```sh
cargo run --example parameter-loading
```

### `profiling`

Demonstrates the profiling module: counting events, opening spans, computing
statistics, and writing profiling data to JSON.

```sh
cargo run --example profiling
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

A network module (using ixa's `network` extentension) which loads a population
with household structure from CSV files and spreads infection along network edges
with different transmission rates by edge type.

```sh
cargo run --example network-hhmodel
```

## External examples

* [ixa-epi-covid](https://github.com/CDCgov/ixa-epi-isolation)
* [ixa-epi-isolation](https://github.com/CDCgov/ixa-epi-isolation)

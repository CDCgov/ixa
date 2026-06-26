# Benchmarking

Ixa's benchmark harness lives in the `ixa-bench` package. The current benchmark
tasks are defined in the top-level `mise.toml`; prefer those tasks over direct
`cargo` commands when running routine benchmarks.

## Setup

Install and activate mise, then trust the repository configuration:

```sh
curl https://mise.run | sh
cd ixa
mise trust mise.toml
```

You can list the available tasks with:

```sh
mise tasks
```

## Running Benchmarks

Run all benchmark suites:

```sh
mise run bench
```

This runs the Hyperfine suite first and then the Criterion suite.

## Hyperfine Benchmarks

Hyperfine benchmarks are registered in `ixa-bench/src` with the
`hyperfine_group!` macro. The current reference SIR comparison is the
`large_sir` group, which compares:

- `baseline`: static reference implementation without Ixa
- `entities`: equivalent Ixa implementation with queries enabled

Run all Hyperfine groups:

```sh
mise run bench:hyperfine
```

Run only the `large_sir` group:

```sh
mise run bench:hyperfine large_sir
```

Run a quick one-pass smoke test of the Hyperfine harness:

```sh
mise run test:hyperfine
```

The `bench:hyperfine` task builds the benchmark binaries first through the
`build:hyperfine` task, then runs `target/release/hyperfine`.

If you need to run one Hyperfine benchmark directly, use the `run_bench` binary:

```sh
cargo run --bin run_bench -p ixa-bench --release -- --group large_sir --bench baseline
cargo run --bin run_bench -p ixa-bench --release -- --group large_sir --bench entities
```

## Criterion Benchmarks

Run all Criterion benchmarks:

```sh
mise run bench:criterion
```

Run a specific Criterion benchmark target:

```sh
mise run bench:criterion sample_entity_scaling
```

The current Criterion benchmark targets are defined in `ixa-bench/Cargo.toml`.
Examples include `examples`, `large_dataset`, `algorithms`, `sampling`,
`indexing`, `counts`, `sample_entity_scaling`, `set_property`, and
`property_semantics`.

The `sample_entity_scaling` target prints a scaling summary for
`sample_entity` cases, including whole-population sampling, indexed
single-property sampling, indexed multi-property sampling, and unindexed
single-property sampling.

## Criterion Baselines

Create a Criterion baseline:

```sh
mise run bench:create --baseline main
```

Create a baseline for one benchmark target:

```sh
mise run bench:create sample_entity_scaling --baseline main
```

Compare against a saved baseline:

```sh
mise run bench:compare --baseline main
```

Compare one benchmark target against a saved baseline:

```sh
mise run bench:compare sample_entity_scaling --baseline main
```

The `bench:compare` task runs `cargo bench` with the selected baseline and then
runs the `check_criterion_regressions` utility.

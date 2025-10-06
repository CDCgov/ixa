# Ixa Profiling and Benchmarking

For general Rust profiling information, see:
[https://nnethercote.github.io/perf-book/profiling.html](https://nnethercote.github.io/perf-book/profiling.html).

## Generating Flamegraphs

You can use `samply` (Linux and macOS) to capture stack samples from your
application and generate a flamegraph, helping you quickly identify and analyze
performance hotspots.

### Prerequisites

Install `samply`:

```bash
cargo install samply
```

### Running

First build in release mode.

```bash
cargo build --example basic-infection --release
```

Then run the resulting binary with `samply`.

```bash
samply record -- target/release/examples/basic-infection
```

You can combine these two commands into one:

```bash
cargo build --example basic-infection --release && samply record -- target/release/examples/basic-infection
```

When it completes, `samply` will automatically open a browser with the generated
report.

### `flamegraph` Alternative

You can use `flamegraph` if you prefer. It requires root privileges, but don't
use `sudo cargo...`. Do this:

```bash
cargo flamegraph --root --example basic-infection
```

This will generate an SVG of the flamegraph in the current directory.

## Benchmarking Ixa

You have two options for benchmarks:

- For micro-benchmarks you want to track over time, use
  [Criterion.rs](https://bheisler.github.io/criterion.rs/book/index.html)
- For in-place, comparisons against a baseline or between multiple
  implementation, use [Hyperfine](https://github.com/sharkdp/hyperfine)

## Criterion

### Optional Prerequisites

- [`gnuplot`](http://www.gnuplot.info/): The
  [plotters crate](https://github.com/38/plotters) will be used as a fallback if
  `gnuplot` is not found.
- [cargo-criterion](https://bheisler.github.io/criterion.rs/book/cargo_criterion/cargo_criterion.html):
  This is the upcoming "next generation" of Criterion.rs. Eventually it will
  reduce compilation times and offer more features, but for now it only has
  feature parity.

```bash
cargo install cargo-criterion
```

### Running Benchmarks

#### Using `cargo bench`

To run all benchmarks:

```bash
cargo bench -p ixa-bench
```

To run a specific benchmark called `example_births_deaths`:

```bash
cargo bench -p ixa-bench --bench example_births_deaths
```

To run a specific named benchmark group named `example_benches`:

```bash
cargo bench -p ixa-bench -- example_benches
```

#### Using `cargo criterion`

To run all benchmarks:

```bash
cargo criterion
```

To run a specific benchmark file called `example_births_deaths`:

```bash
cargo criterion --bench example_births_deaths
```

To run only the benchmarks whose name or group matches `example_benches`:

```bash
cargo criterion -- example_benches
```

#### Viewing Reports

An HTML report is created at `target/criterion/report/index.html`. On macOS:

```bash
open target/criterion/report/index.html
```

On Linux platforms, replace `open` with `xdg-open`, `gnome-open`, or `kde-open`,
depending on your system configuration, or just open the file in a browser.

## Hyperfine

### Prerequisites

You will need to
[install `hyperfine`](https://github.com/sharkdp/hyperfine?tab=readme-ov-file#installation),
e.g. via cargo:

```bash
cargo install hyperfine
```

### Adding a Benchmark Group

Benchmark groups are defined in the `ixa-bench/src` directory. Any included file
that uses the `hyperfine_group!` macro will be registered with the hyperfine
runner.

Here's an example:

```rust
use sort_module::sort_algos;

// To run a registered benchmark, you use its group name.
// e.g. for this group, you would run "just run sort_algorithm"
hyperfine_group!(sort_algorithm, {
    // benchmark comparisons have access to the outer scope of the module
    // as well as anything in the setup scope.
    bubble_sort => {
      sort_algos::bubble_sort(example_data);
    },

    quick_sort => {
      sort_algos::quick_sort(example_data);
    },
});
```

### Running a Benchmark

To list all available benchmarks:

```bash
just hyperfine-list
```

To run a specific benchmark group, e.g. `large_sir`:

```bash
just hyperfine large_sir
```

# Profiling example plan

## Goal

Add a runnable example that demonstrates Ixa’s profiling API end-to-end:

- record named counts and spans during a run
- print profiling tables to stdout
- write a `profiling.json` file using
  `ProfilingContextExt::write_profiling_data()`
- show how `context.report_options()` controls the output path (like the reports
  examples)

## Key constraints (from current implementation)

- Profiling data is global (process-wide). An example should use unique labels
  to avoid collisions.
- `write_profiling_data()` ultimately calls `get_named_*_table()` which requires
  `ProfilingData.start_time` to be set.
  - That only happens after the first `increment_named_count(..)` or
    `open_span(..)`.
  - The example must record at least one count/span before writing the JSON.
- `write_profiling_data()` writes to `<output_dir>/<file_prefix>profiling.json`
  where `output_dir` and `file_prefix` come from `context.report_options()`.

## Example structure (match existing examples)

Create a new cargo example binary:

- `examples/profiling/main.rs` (invoked via `cargo run --example profiling`)

Keep it minimal, like `examples/reports/main.rs`:

- Construct a `Context` directly.
- Configure `context.report_options()` with a stable output directory under the
  repo (or allow overriding).
- Use `ixa::profiling::*` to record a small, deterministic amount of profiling
  data.
- After the run, call `print_profiling_data()` and `write_profiling_data()` on
  the context.

## Implementation steps

### 1) Add `examples/profiling/main.rs`

Behavior:

- Create a `Context` and configure report options, similar to the reports
  examples:
  - `output_dir = $CARGO_MANIFEST_DIR/examples/profiling/output`
  - `file_prefix = "Profiling_"`
  - `overwrite = true` (fine for an example; call out it’s not recommended for
    production)
- During execution, record profiling data with unique labels:
  - `increment_named_count("example_profiling:event")` 3 times (or similar)
  - `let _span = open_span("example_profiling:span")` around a small loop /
    sleep-free work
  - `add_computed_statistic("example_profiling:stat", ..., computer, printer)`
    where:
    - `computer` reads the event count and returns it (so it’s present in JSON)
    - `printer` prints a one-liner
- Ensure the run ends without requiring external inputs:
  - schedule work at `t=0.0`
  - call `Context::shutdown()` at the end of that plan so the queue drains
    quickly

After `context.execute()` returns:

- `ixa::profiling::print_profiling_data()` (optional but useful for the example)
- `context.write_profiling_data()` to write the JSON file (uses
  `context.report_options()` output options)

### 2) Document how to run it (in this file)

Add a short “How to run” section with commands like:

- Run it:
  - `cargo run --example profiling`
- Find the output file:
  - `examples/profiling/output/Profiling_profiling.json`

Notes:

- The example prints the resolved JSON path after writing.
- If you compile with `--no-default-features`, profiling becomes a no-op and the
  JSON file will not be created.

Also mention the feature flag behavior:

- With `--no-default-features`, profiling becomes a no-op (and writing profiling
  data may not make sense).

### 3) Optional: add a quick self-check to the example output

Not required, but helpful:

- after writing the file, read it back and assert it parses as JSON (print a
  friendly error and exit nonzero)
- print the exact output path so users can find it quickly

## Sanity checklist

- Running the example prints a profiling table (counts/spans).
- Running the example creates `<output_dir>/<prefix>profiling.json`.
- JSON contains:
  - `named_counts` entry for `example_profiling:event` with the expected count
  - `named_spans` entry for `example_profiling:span`
  - `computed_statistics` includes `example_profiling:stat` with the expected
    value

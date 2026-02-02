# Profiling integration test plan

## Goal

Add an integration test in `ixa-integration-tests` that verifies profiling data
can be collected and written to disk as JSON via
`ixa::profiling::ProfilingContextExt::write_profiling_data()`.

This test should exercise the real runner configuration path (CLI args →
`BaseArgs` → report options) and then validate that the expected profiling
output file is created and has the expected shape/content.

## What exists today (relevant behavior)

- Profiling is a feature-gated module (`src/profiling/*`), enabled by default
  via the `ixa` crate’s default features (`profiling` is in `default = [...]`).
- Profiling data is stored in a global static container for the process (a
  `OnceLock<Mutex<...>>`).
- `ProfilingContextExt::write_profiling_data()` writes to:
  - directory: `context.report_options().output_dir`
  - filename: `{context.report_options().file_prefix}profiling.json`
  - overwrite behavior: if `overwrite` is `false` and the file exists, it logs
    an error and returns.
- The JSON file content is produced by `src/profiling/file.rs` and includes:
  - `date_time`
  - `execution_statistics`
  - `named_counts` (label/count/rate)
  - `named_spans` (label/count/duration/percent_runtime)
  - `computed_statistics` (map keyed by label, with description/value)

## Test shape (match existing integration tests)

Follow the existing pattern in:

- `integration-tests/ixa-runner-tests/tests/*.rs` (use `assert_cmd` +
  `cargo_bin_cmd!`)
- `integration-tests/ixa-runner-tests/bin/*.rs` (small binaries exercised by the
  tests)

## Implementation plan

### 1) Add a dedicated test binary

Create `integration-tests/ixa-runner-tests/bin/runner_test_profiling.rs`.

Responsibilities:

- Call `ixa::runner::run_with_args(...)` to parse/apply runner args (notably
  `--output`, `--prefix`, `--force-overwrite`) and run a minimal simulation.
- During execution, record some profiling data:
  - `increment_named_count("it_prof_event")` a known number of times (e.g., 3).
  - Open/close a span with a stable label (e.g., `"it_prof_span"`).
  - Optionally register a computed statistic (e.g., total event count for
    `"it_prof_event"`).
- After the simulation completes, call `ctx.write_profiling_data()` so the JSON
  is written using the runner-configured report options.

Suggested minimal structure:

- In `setup_fn`, add a single plan at `t=0.0` that performs the increments +
  span, then calls `Context::shutdown()` so the run terminates quickly.
- After `run_with_args` returns, call
  `ProfilingContextExt::write_profiling_data()` on the returned context.

### 2) Add a new integration test file

Create `integration-tests/ixa-runner-tests/tests/profiling.rs`.

Test responsibilities:

- Create a temporary output directory (`tempfile::tempdir()`).
- Run the binary with args:
  - `--output <tempdir>`
  - `--prefix it_` (or another stable prefix)
  - `--force-overwrite` (so re-runs are safe)
  - optionally `--no-stats` to reduce noise (not required)
- Assert the process exits successfully.
- Assert the expected file exists: `<tempdir>/it_profiling.json`.
- Read and validate content:
  - Prefer JSON parsing (recommended) and assert:
    - `named_counts` contains `label == "it_prof_event"` with `count == 3`
    - `named_spans` contains `label == "it_prof_span"` with `count >= 1`
    - if a computed statistic is added, `computed_statistics["it_prof_stat"]`
      exists and has the expected value

Notes:

- Because this is a separate process (`assert_cmd` launches the test binary),
  the global profiling container starts fresh for the run; you shouldn’t need
  intra-test synchronization.
- Use labels that are unique to this test binary (`it_prof_*`) so future
  additions don’t collide.

### 3) Add any missing dev-dependencies

If the integration test parses JSON, add:

- `serde_json` to `integration-tests/ixa-runner-tests/Cargo.toml` under
  `[dev-dependencies]`.

Alternative (less robust): validate by string matching, but JSON parsing is
preferred.

### 4) Optional follow-ups (separate PRs/tests)

- Overwrite behavior test:
  - Run once with overwrite enabled and verify file exists.
  - Run again with overwrite disabled and verify the binary logs an error and
    the file is unchanged.
- Feature gating test:
  - If the workspace ever disables `ixa` default features for this crate, add
    `features = ["profiling"]` explicitly (or skip the test when profiling is
    off).

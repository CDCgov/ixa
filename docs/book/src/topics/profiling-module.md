# Profiling Module

Ixa includes a lightweight, feature-gated profiling module you can use to:

- Count named events (and compute event rates)
- Time named operations ("spans")
- Print results to the console
- Write results to a JSON file along with execution statistics

The API lives under `ixa::profiling` and is behind the `profiling` Cargo feature
(enabled by default). If you disable the feature, the API becomes a no-op so you
can leave profiling calls in your code.

## Example console output

```text
Span Label                           Count          Duration  % runtime
----------------------------------------------------------------------
load_synth_population                    1       950us 792ns      0.36%
infection_attempt                     1035     6ms 33us 91ns      2.28%
sample_setting                        1035     3ms 66us 52ns      1.16%
get_contact                           1035   1ms 135us 202ns      0.43%
schedule_next_forecasted_infection    1286  22ms 329us 102ns      8.44%
Total Measured                        1385  23ms 897us 146ns      9.03%

Event Label                     Count  Rate (per sec)
-----------------------------------------------------
property progression               36          136.05
recovery                           27          102.04
accepted infection attempt      1,035        3,911.50
forecasted infection            1,286        4,860.09

Infection Forecasting Efficiency: 80.48%
```

## Basic usage

Count an event:

```rust
use ixa::profiling::increment_named_count;

increment_named_count("forecasted infection");
increment_named_count("accepted infection attempt");
```

Time an operation:

```rust
use ixa::profiling::{close_span, open_span};

let span = open_span("forecast loop");
// operation code here (algorithm, function call, etc.)
close_span(span); // optional; dropping the span also closes it
```

Spans also auto-close at end of scope (RAII), which is useful for early returns:

```rust
use ixa::profiling::open_span;

fn complicated_function() {
    let _span = open_span("complicated function");
    // Complicated control flow here, maybe with lots of `return` points.
} // `_span` goes out of scope, automatically closed.
```

Printing results to the console (after the simulation completes):

```rust
use ixa::profiling::print_profiling_data;

print_profiling_data();
```

This prints spans, counts, and any computed statistics. You can also call
`print_named_spans()`, `print_named_counts()`, and `print_computed_statistics()`
individually.

## Minimal example

```rust
use ixa::prelude::*;
use ixa::profiling::*;

fn main() {
    let mut context = Context::new();

    context.add_plan(0.0, |context| {
        increment_named_count("my_model:event");
        {
            let _span = open_span("my_model:expensive_step");
            // ... do work ...
        } // span auto-closes on drop

        context.shutdown();
    });

    context.execute();

    // Console output (spans, counts, computed statistics).
    print_profiling_data();

    // Writes JSON to: <output_dir>/<file_prefix>profiling.json
    // using the same report options configuration as CSV reports.
    context.write_profiling_data();
}
```

See `examples/profiling` in the repository for a more complete example,
including configuring `report_options()` to control the output directory, file
prefix, and overwrite behavior.

## Writing JSON output

`ProfilingContextExt::write_profiling_data()` writes a pretty JSON file to:

`<output_dir>/<file_prefix>profiling.json`

using the same `report_options()` configuration as CSV reports (directory, file
prefix, overwrite). The JSON includes:

- `date_time`
- `execution_statistics`
- `named_counts`
- `named_spans`
- `computed_statistics`

Example:

```rust
use std::path::PathBuf;

use ixa::prelude::*;
use ixa::profiling::ProfilingContextExt;

fn main() {
    let mut context = Context::new();

    context
        .report_options()
        .directory(PathBuf::from("./output"))
        .file_prefix("run_")
        .overwrite(true);

    // ... run the simulation ...
    context.execute();

    context.write_profiling_data();
}
```

## Special names and coverage

Spans may overlap or nest. The sum of all individual span durations will not
generally equal total runtime. A special span named `"Total Measured"` is open
if and only if any other span is open; it tracks how much runtime is covered by
some span.

## Computed statistics

You can register custom, derived metrics over collected `ProfilingData` using
`add_computed_statistic(label, description, computer, printer)`. The "computer"
returns an `Option\<T>` (for conditionally defined statistics), and the "printer"
prints the computed value.

Computed statistics are printed by `print_computed_statistics()` and included in
the JSON under `computed_statistics` (label, description, value).

The supported computed value types are `usize`, `i64`, and `f64`.

API (simplified):

```rust,ignore
pub type CustomStatisticComputer<T> = Box<dyn (Fn(&ProfilingData) -> Option<T>) + Send + Sync>;
pub type CustomStatisticPrinter<T> = Box<dyn Fn(T) + Send + Sync>;

pub fn add_computed_statistic<T: ComputableType>(
    label: &'static str,
    description: &'static str,
    computer: CustomStatisticComputer<T>,
    printer: CustomStatisticPrinter<T>,
);
```

Example:

```rust
use ixa::profiling::{add_computed_statistic, increment_named_count};

increment_named_count("my_model:event");
increment_named_count("my_model:event");

add_computed_statistic::<usize>(
    "my_model:event_count",
    "Total example events",
    Box::new(|data| data.counts.get("my_model:event").copied()),
    Box::new(|value| println!("Computed my_model:event_count = {value}")),
);
```

# Performance and Profiling

## Optimizing Performance with Build Profiles

Build profiles allow you to configure compiler settings for different kinds of builds.
By default, Cargo uses the `dev` profile, which is usually what you want for normal
development of your model but which does not perform optimization. When you are ready to
run a real experiment with your project, you will want to use the `release` build profile,
which does more aggressive code optimization and disables runtime checks for numeric
overflow and debug assertions. In some cases, this can improve performance dramatically.

The [Cargo documentation for build
profiles](https://doc.rust-lang.org/cargo/reference/profiles.html) describes many different
settings you can tweak. You are not limited to Cargo's built in profiles either. In fact, you
might wish to create your own profile for creating flame graphs, for example, as we do in the
section on flame graphs below. These settings go under `[profile.release]` or a custom profile
like `[profile.bench]` in your `Cargo.toml` file. For **maximum execution speed**, the key trio is:

```toml
[profile.release]
opt-level = 3     # Controls the level of optimization. 3 = highest runtime speed. "s"/"z" = size-optimized.
lto = true        # Link Time Optimization. Improves runtime performance by optimizing across crate boundaries.
codegen-units = 1 # Number of codegen units. Lower = better optimization. 1 enables whole-program optimization.
```

The [Cargo documentation for build profiles](https://doc.rust-lang.org/cargo/reference/profiles.html) describes
a few more settings that can affect runtime performance, but these are the most important.

## Visualizing Execution with Flame Graphs

[Samply](https://github.com/mstange/samply/) and
[Flamegraph](https://github.com/flamegraph-rs/flamegraph) are easy to use
profiling tools that generate a "flame graph" that visualizes stack traces,
which allow you to see how much execution time is spent in different parts of
your program. We demonstrate how to use Samply, which has better macOS support.

Install the `samply` tool with Cargo:

```bash
cargo install samply
```

For best results, build your project in both `release` mode and with `debug`
info. The easiest way to do this is to make a build profile, which we name
"profiling" below, by adding the following section to your ` Cargo.toml ` file:

```toml
[profile.profiling]
inherits = "release"
debug = true
```

Now when we build the project we can specify this build profile to Cargo by name:

```bash
cargo build --profile profiling
```

This creates your binary in `target/profiling/my_project`, where `my_project`
is standing in for the name of the project. Now run the project with samply:

```bash
samply record ./target/profiling/my_project
```

We can pass command line arguments as usual if we need to:

```bash
samply record ./target/profiling/my_project arg1 arg2
```

When execution completes, samply will open the results in a browser. The graph looks
something like this:

![Flame Graph](flamegraph.svg)

The graph shows the "stack trace," that is, nested function calls, with a "deeper" function
call stacked on top of the function that called it, but does not otherwise preserve
chronological order of execution. Rather, the width of the function is proportional the time
spent within the function over the course of the entire program execution. Since everything
is ultimately called from your `main` function, you can see `main` at the bottom of the
pile stretching the full width of the graph. This way of representing program execution
allows you to identify "hot spots" where your program is spending most of its time.

## Using Logging to Profile Execution

For simple profiling during development, it is easy to use logging to measure how
long certain operations take. This is especially useful when you want to understand
the cost of specific parts of your application — like loading a large file.

> [!TIP] Cultivate Good Logging Habits
>
> It's good to cultivate the habit of adding `trace!` and `debug!` logging
> messages to your code. You can always selectively enable or disable messages
> for different parts of your program with per-module log level filters. (See
> [the logging module documentation](https://ixa.rs/doc/ixa/log/index.html) for details.)

Suppose we want to know how long it takes to load data for a large population
before we start executing our simulation. We can do this with the following pattern:

```rust
use std::fs::File;
use std::io::BufReader;
use std::time::Instant;
use ixa::trace;

fn load_population_data(path: &str, context: &mut Context) {
    // Record the start time before we begin loading the data.
    let start = Instant::now();

    let file = File::open(path)?;
    let mut reader = BufReader::new(file);
    // .. code to load in the data goes here ...

    // This line computes the time that has elapsed since `start`.
    let duration = start.elapsed();
    trace!("Loaded population data from {} in {:?}", path, duration);
}
```

This pattern is especially useful to pair with a progress bar as in the next section.

## Progress Bar

Provides functions to set up and update a progress bar.

A progress bar has a label, a maximum progress value, and its current progress, which
starts at zero. The maximum and current progress values are constrained to be of type
`usize`. However, convenience methods are provided for the common case of a progress bar
for the timeline that take `f64` time values and rounds them to nearest integers for you.

Only one progress bar can be active at a time. If you try to set a second progress bar, the
new progress bar will replace this first. This is useful if you want to track the progress
of a simulation in multiple phases. Keep in mind, however, that if you define a timeline
progress bar, the `Context` will try to update it in its event loop with the current time,
which might not be what you want if you have replaced the progress bar with a new one.

### Timeline Progress Bar

```rust
/// Initialize the progress bar with the maximum time until the simulation ends.
pub fn init_timeline_progress_bar(max_time: f64);
/// Updates the progress bar with the current time. Finalizes the progress bar when
/// `current_time >= max_time`.
pub fn update_timeline_progress(mut current_time: f64);
```

### Custom Progress Bar

If the timeline is not a good indication of progress for your simulation, you can set up a
custom progress bar.

```rust
/// Initializes a custom progress bar with the given label and max value.
pub fn init_custom_progress_bar(label: &str, max_value: usize);

/// Updates the current value of the custom progress bar.
pub fn update_custom_progress(current_value: usize);

/// Increments the custom progress bar by 1. Use this if you don't want to keep track of the
/// current value.
pub fn increment_custom_progress();
```

### Custom Example: People Infected

Suppose you want a progress bar that tracks how much of the population has been infected (or
infected and then recovered). You first initialize a custom progress bar before executing
the simulation.

```rust
use crate::progress_bar::{init_custom_progress_bar};

init_custom_progress_bar("People Infected", POPULATION_SIZE);
```

To update the progress bar, we need to listen to the infection status property change event.

```rust
use crate::progress_bar::{increment_custom_progress};

// You might already have this event defined for other purposes.
pub type InfectionStatusEvent = PersonPropertyChangeEvent<InfectionStatus>;

// This will handle the status change event, updating the progress bar
// if there is a new infection.
fn handle_infection_status_change(context: &mut Context, event: InfectionStatusEvent) {
  // We only increment the progress bar when a new infection occurs.
  if (InfectionStatusValue::Susceptible, InfectionStatusValue::Infected)
      == (event.previous, event.current)
  {
    increment_custom_progress();
  }
}

// Be sure to subscribe to the event when you initialize the context.
pub fn init(context: &mut Context) -> Result<(), IxaError> {
    // ... other initialization code ...
    context.subscribe_to_event::<InfectionStatusEvent>(handle_infection_status_change);
    // ...
    Ok(())
}
```

## Additional Resources

For an in-depth look at performance in Rust programming, including
many advanced tools and techniques, check out [The Rust Performance
Book](https://nnethercote.github.io/perf-book/title-page.html).

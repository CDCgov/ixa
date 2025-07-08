# The Incident Reporter

An agent-based model does not output an answer at the end of a simulation in the usual sense. Rather, the simulation evolves the state of the world over time. If we want to track that evolution for later analysis, it is up to us to collect the data we want to have. The built-in report feature makes it easy to record data to a CSV file during the simulation.

Our model will only have a single report that records the current in-simulation time, the `PersonId`, and the `InfectionStatusValue` of a person whenever their `InfectionStatus` changes. We define a struct representing a single row of data.

```rust
// in incidence_report.rs
{{#rustdoc_include ../../models/disease_model/src/incidence_report.rs:IncidenceReportItem}}
```

The fact that `IncidenceReportItem` derives `Serialize` is what makes this magic work. We define a report for this struct using the `define_report!` macro.

```rust
{{#rustdoc_include ../../models/disease_model/src/incidence_report.rs:define_report}}
```

The way we listen to events is almost identical to how we did it in the `infection` module. First let's make the event handler, that is, the callback that will be called whenever an event is emitted.

```rust
{{#rustdoc_include ../../models/disease_model/src/incidence_report.rs:handle_infection_status_change}}
```

Just pass a `IncidenceReportItem` to `context.send_report()`! We also emit a trace log message so we can trace the execution of our model.

In the `init()` function there is a little bit of setup needed. Also, we can't forget to register this callback to listen to `InfectionStatusEvent`s.

```rust
{{#rustdoc_include ../../models/disease_model/src/incidence_report.rs:init}}
```

Note that:

- the configuration you do on `context.report_options()` applies to all reports attached to that context;
- using `overwrite(true)` is useful for debugging but potentially devastating for production;
- this `init()` function returns a result, which will be whatever error that `context.add_report()` returns if the CSV file cannot be created for some reason, or `Ok(())` otherwise.

> [!INFO] `Result<U, V>` and Handling Errors
> The Rust `Result<U, V>` type is an enum used for error handling. It represents a value that can either be a successful outcome (`Ok`) containing a value of type `U`, or an error (`Err`) containing a value of type `V`. Think of it as a built-in way to return and propagate errors without relying on exceptions, similar to using “`Either`” types or special error codes in other languages.
>
> The `?` operator works with `Result` to simplify error handling. When you append `?` to a function call that returns a `Result`, it automatically checks if the result is an `Ok` or an `Err`. If it’s `Ok`, the value is extracted; if it’s an `Err`, the error is immediately returned from the enclosing function. This helps keep your code concise and easy to read by reducing the need for explicit error-checking logic.

If your IDE isn't capable of adding imports for you, the external symbols we need for this module are as follows.

```rust
{{#rustdoc_include ../../models/disease_model/src/incidence_report.rs:imports}}
```

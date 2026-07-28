# Get Started

If you are new to Rust, we suggest taking some time to learn the parts of Rust
that are most useful for Ixa development. We've compiled some resources
[in rust-resources.md](appendix_rust/rust-resources.md).

Execute the following commands to create a new Rust project called `ixa_model`.

```bash
cargo new --bin ixa_model

cd ixa_model
```

Use Ixa's new project setup script to set up the project for Ixa. By default,
the script installs the blank starter used by this guide.

```bash
curl -s -f -L https://raw.githubusercontent.com/CDCgov/ixa/main/scripts/setup_new_ixa_project.sh | sh
```

Open `src/main.rs` in your favorite editor or IDE to verify the model looks like
the following:

```rust
{{#rustdoc_include ../models/basic/main.rs}}
```

To run the model:

```bash
cargo run
# The current time is 1
```

To run with logging enabled globally:

```bash
cargo run -- --log-level=trace
```

To run with logging enabled for just `ixa_model`:

```bash
cargo run -- --log-level ixa_model=trace
```

## Other templates

When creating a new project, you can select the `parameters` template to start
with a parameters module, parameter validation, and basic global property
integration:

```bash
curl -s -f -L https://raw.githubusercontent.com/CDCgov/ixa/main/scripts/setup_new_ixa_project.sh |
  sh -s -- --template parameters
```

This template creates `src/main.rs` and `src/parameters.rs`. Running
`cargo run` validates and stores the default parameters, then prints:

```text
The current time is 100
```

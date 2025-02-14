# Ixa

⚠️ This is a work in progress

## Getting started

While working in VS Code, we suggest first installing a few extensions:

- rust
- Dependi

## Examples

Working from the root `ixa` directory, models in the `examples` folder can be built and run using the command:

```
cargo run --example <example-name>
```

## As a crate

`ixa` can also be installed as a crate (a.k.a. library in Rust) in other Rust projects. After creating a new Rust project using

```
cargo new <project-name>
```

you can add `ixa` dependencies to the project's Cargo.toml:

```
[dependencies]
ixa = { git = "https://github.com/cdcgov/ixa.git", version = "0.0.1" }
ixa-derive = { git = "https://github.com/cdcgov/ixa.git", version = "0.0.0" }
```

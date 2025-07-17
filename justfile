########################################
# Setup Tasks (one-time developer use)
########################################

# Install the wasm32 target for Rust
install-wasm-target:
    rustup target add wasm32-unknown-unknown

# Install wasm-pack if not already installed
install-wasm-pack:
    command -v wasm-pack >/dev/null || cargo install wasm-pack --locked

# Install JS dependencies and Playwright binaries
install-playwright:
    cd integration-tests/ixa-wasm-tests && \
    npm install && \
    npx playwright install --with-deps

# Install mdBook and its required plugins
install-mdbook:
    cargo install mdbook
    cargo install mdbook-callouts
    cargo install mdbook-inline-highlighting

# Install pre-commit and set up Git hooks
install-pre-commit:
    command -v pre-commit >/dev/null || pip install --user pre-commit
    pre-commit install

# Install markdownlint
install-markdownlint:
    npm install -g markdownlint-cli

# Install or upgrade Prettier for formatting Markdown
install-prettier:
    npm install -g prettier

# install all
install:
    just install-wasm-target
    just install-wasm-pack
    just install-playwright
    just install-mdbook
    just install-pre-commit
    just install-markdownlint
    just install-prettier

########################################
# Build Tasks
########################################

# Build the default Rust targets (bin and lib) for all workspace members
build:
    cargo build --workspace --verbose

# Build the Wasm target with logging enabled and no default features as a check. Use `build-wasm-pack` instead.
build-wasm:
    cargo build --verbose --target wasm32-unknown-unknown --no-default-features --features logging

# Build the wasm module for the browser using wasm-pack
build-wasm-pack:
    cd integration-tests/ixa-wasm-tests && wasm-pack build --target web

# Ensure benchmark targets compile
bench-build:
    cargo bench -p ixa-bench --no-run

# Build all examples
build-examples:
    cargo build --verbose --examples

# Build all tests without running them
build-tests:
    cargo test --no-run --verbose

# Build all targets: lib, bin, test, example, bench. Does NOT build wasm/wasm-pack
build-all:
    cargo build --all-targets --workspace --verbose

########################################
# Test and Benchmark Tasks
########################################

# Run all unit and integration tests for all packages (excludes examples and wasm)
test:
    cargo test --workspace --verbose

# Run ixa example tests
test-examples:
    cargo test  --workspace --examples

# Run all benchmarks
bench:
    cargo bench -p ixa-bench

# Run browser-based Playwright tests via npm
test-playwright:
    cd integration-tests/ixa-wasm-tests && npm test

# Alias: wasm-test is a clearer name for Playwright-based Wasm tests
test-wasm: test-playwright

########################################
# Example Run Tasks
########################################
# Run individual example binaries

# Run the `basic` example
run-example-basic:
    cargo run --example basic

# Run the `basic-infection` example
run-example-basic-infection:
    cargo run --example basic-infection

# Run the `births-deaths` example
run-example-births-deaths:
    cargo run --example births-deaths

# Run the `load-people` example
run-example-load-people:
    cargo run --example load-people

# Run the `network-hhmodel` example
run-example-network-hhmodel:
    cargo run --example network-hhmodel

# Run the `parameter-loading` example
run-example-parameter-loading:
    cargo run --example parameter-loading

# Run the `random` example
run-example-random:
    cargo run --example random

# Run the `reports` example
run-example-reports:
    cargo run --example reports

# Run the `reports-multi-threaded` example
run-example-reports-multi-threaded:
    cargo run --example reports-multi-threaded

# Run the `runner` example
run-example-runner:
    cargo run --example runner

# Run the `time-varying-infection` example
run-example-time-varying-infection:
    cargo run --example time-varying-infection -- examples/time-varying-infection/input.json

# Run all example binaries
run-examples:
    just run-example-basic
    just run-example-basic-infection
    just run-example-births-deaths
    just run-example-load-people
    just run-example-network-hhmodel
    just run-example-parameter-loading
    just run-example-random
    just run-example-reports
    just run-example-reports-multi-threaded
    just run-example-runner
    just run-example-time-varying-infection

########################################
# Documentation Tasks
########################################

# Build the Rust API docs into the website/ directory
build-docs:
    cargo doc --no-deps --target-dir website/

# Build the Ixa Book into website/book/
build-book:
    mdbook build docs/book -d ../../website/book

########################################
# Linting, Formatting, and Code Quality
########################################

# Run all pre-commit checks (as configured in .pre-commit-config.yaml)
precommit:
    pre-commit run --all-files

# Lint all Markdown files (no fixes)
lint-md:
    markdownlint "**/*.md"

# Auto-fix and format all Markdown files
fix-md:
    markdownlint --fix "**/*.md"
    prettier --write "**/*.md" --ignore-path ./.gitignore --ignore-path ./.markdownlintignore

# Lint a specific Markdown file
lint-md-file filename:
    markdownlint {{filename}}

# Fix and format a specific Markdown file
fix-md-file filename:
    markdownlint --fix {{filename}}
    prettier --write {{filename}}

# Format all Rust code in the workspace using rustfmt
format-rust:
    cargo fmt

# Lint the entire Rust workspace using Clippy (no auto-fix)
lint-rust:
    cargo clippy --workspace --all-targets -- -D warnings

# Attempt to auto-fix Clippy lints (Rust nightly only)
fix-rust:
    cargo clippy --fix --workspace --all-targets --allow-dirty -- -D warnings

########################################
# Clean Tasks
########################################

# Remove Rust build artifacts (`cargo clean`)
clean-target:
    cargo clean

# Remove wasm-pack and Playwright build artifacts (including `node_modules`)
clean-wasm:
    rm -rf pkg
    rm -rf integration-tests/ixa-wasm-tests/pkg
    rm -rf integration-tests/ixa-wasm-tests/node_modules

# Remove all documentation artifacts
clean-docs:
    rm -rf website/doc website/debug
    rm -f website/.rustc_info.json website/.rustdoc_fingerprint.json

# Remove all book artifacts
clean-book:
    rm -rf website/book

# Delete example-generated output files and directories
clean-examples:
    rm -f \
        Reports_death.csv \
        Reports_incidence.csv \
        examples/births-deaths/incidence.csv \
        examples/births-deaths/people_report.csv \
        examples/parameter-loading/incidence.csv \
        examples/reports-multi-threaded/Arizona_incidence.csv \
        examples/reports-multi-threaded/California_incidence.csv \
        examples/reports-multi-threaded/Illinois_incidence.csv \
        examples/reports-multi-threaded/Wisconsin_incidence.csv \
        examples/time-varying-infection/incidence.csv \
        examples/time-varying-infection/person_property_count.csv \
        incidence.csv \
        people_report.csv
    rm -rf \
        examples/network-hhmodel/output/ \
        examples/network-hhmodel/tests/

# Remove all build artifacts (all `clean*` recipes)
clean:
    just clean-target
    just clean-wasm
    just clean-docs
    just clean-book
    just clean-examples

########################################
# CI Task (all checks and builds)
########################################

# Main CI task: run everything expected in CI (except setup/install)
ci:
    just precommit
    just build-all
    just build-wasm
    just test
    just test-examples
    just run-examples
    just build-wasm-pack
    just test-wasm
    just build-docs
    just build-book

########################################
# Setup Tasks (one-time developer use)
########################################

# Install the wasm32 target for Rust
install-wasm-target:
    rustup target add wasm32-unknown-unknown

# Install wasm-pack if not already installed
install-wasm-pack:
    if ! command -v wasm-pack &> /dev/null; then
        cargo install wasm-pack --locked
    fi

# Install JS dependencies and Playwright binaries
playwright-setup:
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
    if ! command -v pre-commit &> /dev/null; then
        pip install --user pre-commit
        pre-commit install
    fi

########################################
# Build Tasks
########################################

# Build the default Rust targets (bin and lib) for all workspace members
build:
    cargo build --workspace --verbose

# Build the Wasm target with logging enabled and no default features.
# Use `build-wasm-pack` instead.
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

# Build all targets: lib, bin, test, example, bench
# Does NOT build wasm/wasm-pack
build-all:
    cargo build --all-targets --workspace --verbose

########################################
# Test and Benchmark Tasks
########################################

# Run all unit and integration tests
test:
    cargo test --workspace --verbose

# Run example tests
test-examples:
    cargo test --examples

# Run all benchmarks
bench:
    cargo bench -p ixa-bench

# Run browser-based Playwright tests via npm
playwright-test:
    cd integration-tests/ixa-wasm-tests && npm test

# Alias: wasm-test is a clearer name for Playwright-based Wasm tests
wasm-test: playwright-test

########################################
# Example Run Tasks
########################################

# Run individual example binaries
run-example-basic:
    cargo run --example basic

run-example-basic-infection:
    cargo run --example basic-infection

run-example-births-deaths:
    cargo run --example births-deaths

run-example-load-people:
    cargo run --example load-people

run-example-network-hhmodel:
    cargo run --example network-hhmodel

run-example-parameter-loading:
    cargo run --example parameter-loading

run-example-random:
    cargo run --example random

run-example-reports:
    cargo run --example reports

run-example-reports-multi-threaded:
    cargo run --example reports-multi-threaded

run-example-runner:
    cargo run --example runner

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
# Linting and Code Quality
########################################

# Run all pre-commit checks (as configured in .pre-commit-config.yaml)
precommit:
    pre-commit run --all-files

# Run markdownlint-cli2 manually (not yet enabled in pre-commit)
markdownlint:
    markdownlint-cli2 "**/*.md"

########################################
# Clean Tasks
########################################

# Remove Rust build artifacts
clean-target:
    cargo clean

# Remove wasm-pack and Playwright build artifacts
clean-wasm:
    rm -rf pkg
    rm -rf integration-tests/ixa-wasm-tests/pkg
    rm -rf integration-tests/ixa-wasm-tests/node_modules

# Remove all documentation artifacts
clean-docs:
    rm -rf website/doc

clean-book:
    rm -rf website/book

# Remove all build artifacts
clean:
    just clean-target
    just clean-docs
    just clean-book
    just clean-wasm

########################################
# CI Task (all checks and builds)
########################################

# Main CI task: run everything expected in CI (except setup/install)
ci:
    just precommit
    just markdownlint
    just build-all
    just build-wasm
    just test
    just test-examples
    just run-examples
    just wasm-test
    just build-docs
    just build-book

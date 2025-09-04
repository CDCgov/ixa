# List all available recipes
default:
    just --list

########################################
# Wasm related tasks
########################################

# Install the wasm32 target for Rust
[group('Install')]
[group('Wasm')]
install-wasm-target:
    rustup target add wasm32-unknown-unknown

# Install wasm-pack if not already installed
[group('Install')]
[group('Wasm')]
install-wasm-pack: install-wasm-target
    command -v wasm-pack >/dev/null || cargo install wasm-pack --locked

# Install JS dependencies and Playwright binaries
[group('Install')]
[group('Wasm')]
install-playwright:
    cd integration-tests/ixa-wasm-tests && \
    npm install && \
    npx playwright install --with-deps

# Build the wasm module for the browser using wasm-pack
[group('Wasm')]
[group('Build')]
build-wasm-pack: install-wasm-pack
    cd integration-tests/ixa-wasm-tests && wasm-pack build --target web

# Run browser-based Playwright tests via npm
[group('Bench')]
[group('Wasm')]
test-wasm: install-playwright build-wasm-pack
    cd integration-tests/ixa-wasm-tests && npm test

# Remove wasm-pack and Playwright build artifacts (including `node_modules`)
[group('Wasm')]
[group('Clean')]
clean-wasm:
    rm -rf pkg
    rm -rf integration-tests/ixa-wasm-tests/pkg
    rm -rf integration-tests/ixa-wasm-tests/node_modules

########################################
# Ixa Book & Docs
########################################

# Install mdBook and its required plugins
[group('Install')]
[group('Ixa Book & Docs')]
install-mdbook:
    cargo install mdbook mdbook-callouts mdbook-inline-highlighting

# Install markdownlint
[group('Install')]
[group('Ixa Book & Docs')]
install-markdownlint:
    npm install -g markdownlint-cli

# Install or upgrade Prettier for formatting Markdown
[group('Install')]
[group('Ixa Book & Docs')]
install-prettier:
    npm install -g prettier

# Build the Rust API docs into the website/ directory
[group('Build')]
[group('Ixa Book & Docs')]
build-docs:
    cargo doc --no-deps --target-dir website/

# Build the Ixa Book into website/book/
[group('Build')]
[group('Ixa Book & Docs')]
build-book: install-mdbook
    mdbook build docs/book -d ../../website/book

# Lint all Markdown files (no fixes)
[group('Ixa Book & Docs')]
[group('Lint')]
lint-md:
    markdownlint "**/*.md"

# Auto-fix and format all Markdown files
[group('Ixa Book & Docs')]
[group('Lint')]
fix-md: install-markdownlint install-prettier
    markdownlint --fix "**/*.md" || true
    prettier --write "**/*.md" --ignore-path ./.gitignore --ignore-path ./.markdownlintignore

# Lint a specific Markdown file
[no-cd]
[group('Ixa Book & Docs')]
[group('Lint')]
lint-md-file filename: install-markdownlint
    markdownlint {{filename}}

# Fix and format a specific Markdown file
[no-cd]
[group('Ixa Book & Docs')]
[group('Lint')]
fix-md-file filename: install-markdownlint install-prettier
    markdownlint --fix {{filename}} || true
    prettier --write {{filename}}

# Remove all documentation artifacts
[group('Ixa Book & Docs')]
[group('Clean')]
clean-docs:
    rm -rf website/doc website/debug
    rm -f website/.rustc_info.json website/.rustdoc_fingerprint.json

# Remove all book artifacts
[group('Ixa Book & Docs')]
[group('Clean')]
clean-book:
    rm -rf website/book

########################################
# Setup Tasks (one-time developer use)
########################################

# Install pre-commit and set up Git hooks
[group('Install')]
install-pre-commit:
    command -v pre-commit >/dev/null || pip install --user pre-commit
    pre-commit install

# Install all
[group('Install')]
install: install-wasm-target install-wasm-pack install-playwright install-mdbook install-pre-commit \
         install-markdownlint install-prettier

########################################
# Build Tasks
########################################

# Build the default Rust targets (bin and lib) for all workspace members
[group('Build')]
build:
    cargo build --workspace --verbose

# Build all targets for workspace: lib, bin, test, example, bench. Does NOT build wasm/wasm-pack
[group('Build')]
build-all-targets:
    cargo build --all-targets --workspace --verbose

########################################
# Test and Benchmark Tasks
########################################

# Run all unit and integration tests for all packages (excluding examples and wasm)
[group('Tests')]
test:
    cargo test --workspace --verbose

# Run all benchmarks
[group('Bench')]
bench:
    cargo bench -p ixa-bench

# Create a new named benchmark baseline
[group('Bench')]
baseline-create name:
    @echo "Creating new Criterion baseline: {{name}}"
    cargo bench -p ixa-bench -- --save-baseline {{name}}

# Run benchmarks compared against an existing named baseline
[group('Bench')]
baseline-compare name:
    @echo "Running benchmarks compared to baseline: {{name}}"
    cargo bench -p ixa-bench -- --baseline {{name}}

# Build benchmark targets
[group('Build')]
[group('Bench')]
build-bench:
    cargo bench -p ixa-bench --no-run

# Build all tests in workspace without running them
[group('Build')]
[group('Tests')]
build-tests:
    cargo test --workspace --no-run --verbose

########################################
# Examples Run Tasks
########################################

# Run a named example
[group('Examples')]
run-example name:
    cargo run --example "{{name}}"

# Run all example binaries
[group('Examples')]
run-examples:
    @for example in $(cargo build --example 2>&1 | tail -n +3); do \
        just run-example "$example"; \
    done

# Run ixa example tests
[group('Examples')]
[group('Tests')]
test-examples:
    cargo test  --workspace --examples

# Build all examples in workspace
[group('Examples')]
[group('Build')]
build-examples:
    cargo build --workspace --examples --verbose


# Delete example-generated output files
[group('Examples')]
[group('Clean')]
clean-examples:
    @for example in $(cargo build --example 2>&1 | tail -n +3); do \
        rm -f ./examples/"$example"/output/*.csv; \
    done

########################################
# Linting, Formatting, and Code Quality
########################################

# Run all pre-commit checks (as configured in .pre-commit-config.yaml)
[group('Lint')]
precommit:
    pre-commit run --all-files

# Format all Rust code in the workspace using rustfmt
[group('Lint')]
format-rust:
    cargo fmt

# Lint the entire Rust workspace using Clippy (no auto-fix)
[group('Lint')]
lint-rust:
    cargo clippy --workspace --all-targets -- -D warnings

# Attempt to auto-fix Clippy lints (Rust nightly only)
[group('Lint')]
fix-rust:
    cargo clippy --fix --workspace --all-targets --allow-dirty -- -D warnings

########################################
# Clean Tasks
########################################

# Remove Rust build artifacts (`cargo clean`)
[group('Clean')]
clean-target:
    cargo clean

# Remove all build artifacts (all `clean*` recipes)
[group('Clean')]
clean: clean-target clean-wasm clean-docs clean-book clean-examples

########################################
# CI Task (all checks and builds)
########################################

# In the GitHub Workflow, tasks are run individually. This recipe is for running the CI tasks in a local dev
# environment, say, before pushing.

# Run locally everything that runs in CI
prepush $RUSTFLAGS="-D warnings": precommit build-all-targets build-wasm-pack test test-examples test-wasm \
                                      run-examples build-docs build-book

########################################
# Docker testing in container
########################################

# Run bench in Docker
docker-run:
    docker build -t ixa-bench .
    docker run --rm -it -v "$PWD":/home/runner/ixa ixa-bench

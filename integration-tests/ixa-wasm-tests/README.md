# Ixa WASM Integration Tests

Integration tests for [Ixa](https://github.com/CDCgov/ixa) compiled to
WebAssembly and run in a browser using Playwright.

## Requirements

- [wasm-pack](https://rustwasm.github.io/wasm-pack/)
- Node.js and pnpm
- Playwright browsers (`pnpm exec playwright install`)

## Commands

Install dependencies:

```sh
pnpm install
```

Run the full test suite (builds WASM and runs Playwright tests):

```sh
pnpm test
```

Start a local dev server (useful for manual testing in the browser):

```sh
pnpm start
```

## How it works

`pnpm test` runs two steps:

1. `wasm-pack build --target web` — compiles the Rust crate to WASM (output in `pkg/`)
2. `playwright test` — launches a browser, serves `index.html`, and runs the tests in `tests/`

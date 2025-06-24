# 🧪 Ixa WASM Integration Tests

This directory contains integration tests for the [Ixa](https://github.com/CDCgov/ixa)
project compiled to WebAssembly and run in a browser using Playwright.

The [`build.sh`](./build.sh) script is the primary entry point for building, serving, testing, and
cleaning the project. It supports multiple modes, making it easy to run tests locally or in CI.

## 🚀 `build.sh` Usage

Run the script with one of the following options:

```sh
./build.sh              # Default: install + build + test
./build.sh test         # Build and test only (no install)
./build.sh serve        # Build and start a local dev server (no test)
./build.sh clean        # Remove generated files
```

### 🔧 Command Summary

| **Command**      | **Installs deps** | **Builds WASM** | **Starts Server** | **Runs Tests** | **Shuts Down Server** |
| ---------------- |-------------------|-----------------| ----------------- | -------------- | --------------------- |
| ./build.sh       | ✅ Yes             | ✅ Yes           | ✅ Yes             | ✅ Yes          | ✅ Yes                 |
| ./build.sh test  | ❌ No              | ✅ Yes           | ✅ Yes             | ✅ Yes          | ✅ Yes                 |
| ./build.sh serve | ❌ No              | ✅ Yes           | ✅ Yes             | ❌ No           | ❌ No                  |
| ./build.sh clean | ❌ No              | ❌ No            | ❌ No              | ❌ No           | ❌ N/A                 |

## 📝 Requirements

- Node.js and npm
- wasm-pack
- Either:
  - http-server (`npm install -g http-server`), installed with deps.
  - Or Python 3 for fallback static server

The default command automatically installs playwright and other NPM dependencies.

## 🧹 Cleaning Up

To remove generated and temporary files, run:

```sh
./build.sh clean
```

This deletes:

- `pkg/` – WASM build output
- `node_modules/` – NPM dependencies
- `test-results/` – Playwright output

## 🧪 Playwright Tests

Tests are located in the tests/ directory. They run automatically when using:

```sh
./build.sh
# or
./build.sh test
```

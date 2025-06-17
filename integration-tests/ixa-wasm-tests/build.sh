#!/bin/bash

set -euo pipefail

# Save the original directory and restore it on exit
ORIG_DIR="$(pwd)"
cd "$(dirname "$0")"

trap cleanup EXIT

# Constants
PORT=8080

export RUSTFLAGS='--cfg getrandom_backend="wasm_js"'

# Install npm if it's not available
check_npm_installed() {
    if ! command -v npm &> /dev/null; then
        echo "📦 npm not found. Attempting to install..."
        if command -v brew &> /dev/null; then
            brew install node
        else
            echo "❌ Cannot install npm automatically (no brew). Please install Node.js manually."
            exit 1
        fi
    fi
}

# Install NPM dependencies
install_npm_deps() {
    echo "📦 Installing npm dependencies..."
    npm install
}

# Build wasm
build_wasm() {
    echo "🔧 Building WASM with wasm-pack..."
    wasm-pack build --target web
}

# Start a server in the background
start_server() {
    echo "🚀 Starting local server in background..."
    if command -v http-server &> /dev/null; then
        http-server . -p $PORT > /dev/null 2>&1 &
        SERVER_PID=$!
    elif command -v python3 &> /dev/null; then
        python3 -m http.server $PORT > /dev/null 2>&1 &
        SERVER_PID=$!
    else
        echo "❌ No HTTP server found. Please install http-server (npm) or use Python 3."
        exit 1
    fi
}

# Run tests
run_tests() {
    echo "🧪 Running Playwright tests..."
    npx playwright test
}

# Cleanup function to kill server
cleanup() {
    if [[ -n "${SERVER_PID-}" ]]; then
        echo "🛑 Stopping local server (pid $SERVER_PID)..."
        kill $SERVER_PID
    fi
    cd "$ORIG_DIR"
}

# Clean generated files
clean_artifacts() {
    echo "🧹 Cleaning generated files..."
    rm -rf pkg/ node_modules/ test-results/
    echo "✅ Clean complete."
}

# Entry point
case "${1-}" in
    serve)
        build_wasm
        start_server
        ;;
    test)
        build_wasm
        run_tests
        ;;
    clean)
        clean_artifacts
        ;;
    *)
        check_npm_installed
        install_npm_deps
        build_wasm
        run_tests
        ;;
esac

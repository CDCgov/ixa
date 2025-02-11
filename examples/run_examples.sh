#!/bin/bash

# List of commands to run
commands=(
    "cargo run --example basic-infection"
    "cargo run --example births-deaths"
    "cargo run --example load-people"
    "cargo run --example network-hhmodel"
    "cargo run --example parameter-loading"
    "cargo run --example random"
    "cargo run --example reports"
    "cargo run --example reports-multi-threaded"
    "cargo run --example runner"
    "cargo run --example time-varying-infection ./examples/time-varying-infection/input.json"
)

# Iterate over the commands and run them
for cmd in "${commands[@]}"; do
    echo "Running: $cmd"
    $cmd
    # Check the exit code of the last command
    if [ $? -ne 0 ]; then
        echo "Command failed: $cmd"
        exit 1
    fi
done

echo "All commands executed successfully."
exit 0

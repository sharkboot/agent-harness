#!/bin/bash
# Test script for Agent Harness

set -euo pipefail

echo "Running tests..."

# Run Rust tests
echo "Running Rust tests..."
cargo test --all

# Run TypeScript tests
echo "Running TypeScript tests..."
if [ -f "packages/ui-components/package.json" ]; then
    cd packages/ui-components
    npm test
    cd ../..
fi

echo "All tests passed!"

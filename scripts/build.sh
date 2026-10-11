#!/bin/bash
# Build script for Agent Harness

set -euo pipefail

echo "Building Agent Harness..."

# Build Rust workspace
echo "Building Rust crates..."
cargo build --release

# Build TypeScript packages
echo "Building TypeScript packages..."
if [ -f "package.json" ]; then
    cd packages/ui-components
    npm run build
    cd ../..
fi

# Run tests
echo "Running tests..."
cargo test --all

echo "Build complete!"

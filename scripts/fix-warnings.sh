#!/bin/bash

# Script to fix common warnings in the Supernova codebase

echo "Fixing common warnings in the Supernova codebase..."

# Fix unused imports
echo "Removing unused imports..."
cargo fix --allow-dirty --allow-staged 2>/dev/null || true

# Run cargo fmt to fix formatting issues
echo "Running cargo fmt..."
cargo fmt --all

# Show remaining warnings
echo "Checking for remaining warnings..."
cargo check 2>&1 | grep -E "warning:" | head -20

echo "Done! Check the output above for any remaining warnings that need manual fixing." 
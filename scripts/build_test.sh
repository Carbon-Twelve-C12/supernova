#!/bin/bash

# Script to test building and compiling core components

set -e  # Exit on error

# Color variables for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
NC='\033[0m' # No Color

echo -e "${YELLOW}===== Building SuperNova core library =====${NC}"

# First try to compile the base library
echo -e "${YELLOW}Compiling btclib...${NC}"
cargo build -p btclib || {
    echo -e "${RED}Failed to build btclib${NC}"
    exit 1
}

echo -e "${GREEN}Successfully built btclib${NC}"

# Test individual source files separately - helpful to isolate issues
echo -e "${YELLOW}Testing each major module independently...${NC}"

declare -a modules=(
    "crypto/quantum.rs"
    "environmental/emissions.rs"
    "environmental/treasury.rs"
    "environmental/dashboard.rs"
    "environmental/miner_reporting.rs"
    "validation.rs"
)

for module in "${modules[@]}"; do
    echo -e "${YELLOW}Checking btclib/src/${module}...${NC}"
    rustc --cfg test --test btclib/src/${module} -o /dev/null 2>/dev/null || {
        echo -e "${RED}Failed to compile ${module}${NC}"
        # Continue to check other modules
    }
done

echo -e "${YELLOW}===== Building test examples =====${NC}"

# Try to build example files
echo -e "${YELLOW}Compiling quantum_test example...${NC}"
cargo build --example quantum_test || {
    echo -e "${RED}Failed to build quantum_test example${NC}"
    exit 1
}

echo -e "${GREEN}Successfully built quantum_test example${NC}"

echo -e "${YELLOW}Compiling environmental_demo example...${NC}"
cargo build --example environmental_demo || {
    echo -e "${RED}Failed to build environmental_demo example${NC}"
    exit 1
}

echo -e "${GREEN}Successfully built environmental_demo example${NC}"

echo -e "${GREEN}All core components built successfully!${NC}" 
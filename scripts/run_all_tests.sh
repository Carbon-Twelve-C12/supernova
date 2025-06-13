#!/bin/bash
# Comprehensive Test Suite for Supernova
# Meeting the Satoshi Standard: All tests must pass

set -e  # Exit on any error

echo "🚀 Running Supernova Comprehensive Test Suite"
echo "============================================"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Test counters
TOTAL_TESTS=0
PASSED_TESTS=0
FAILED_TESTS=0

# Function to run tests and track results
run_test_suite() {
    local suite_name=$1
    local test_command=$2
    
    echo -e "\n${YELLOW}Running $suite_name...${NC}"
    
    if $test_command; then
        echo -e "${GREEN}✓ $suite_name passed${NC}"
        ((PASSED_TESTS++))
    else
        echo -e "${RED}✗ $suite_name failed${NC}"
        ((FAILED_TESTS++))
    fi
    ((TOTAL_TESTS++))
}

# Clean previous test artifacts
echo "Cleaning test artifacts..."
rm -rf target/debug/deps/*test*
rm -rf /tmp/supernova_test*

# 1. Unit Tests
echo -e "\n${YELLOW}=== UNIT TESTS ===${NC}"
run_test_suite "btclib unit tests" "cargo test -p btclib --lib"
run_test_suite "node unit tests" "cargo test -p node --lib"
run_test_suite "miner unit tests" "cargo test -p miner --lib"
run_test_suite "wallet unit tests" "cargo test -p wallet --lib"
run_test_suite "cli unit tests" "cargo test -p cli --lib"

# 2. Integration Tests
echo -e "\n${YELLOW}=== INTEGRATION TESTS ===${NC}"
run_test_suite "Full integration tests" "cargo test --test integration_tests"
run_test_suite "Lightning integration" "cargo test --test lightning_integration"
run_test_suite "Quantum security tests" "cargo test --test quantum_integration"

# 3. Doc Tests
echo -e "\n${YELLOW}=== DOCUMENTATION TESTS ===${NC}"
run_test_suite "Documentation tests" "cargo test --doc"

# 4. Benchmarks (run but don't fail on performance)
echo -e "\n${YELLOW}=== BENCHMARKS ===${NC}"
echo "Running benchmarks (informational only)..."
cargo bench --no-fail-fast || true

# 5. Security Audit
echo -e "\n${YELLOW}=== SECURITY AUDIT ===${NC}"
echo "Checking for unsafe code..."
if grep -r "unsafe" --include="*.rs" btclib/ node/ miner/ wallet/ | grep -v "// SAFETY:" | grep -v "test"; then
    echo -e "${RED}Found unsafe code without SAFETY comments${NC}"
    ((FAILED_TESTS++))
else
    echo -e "${GREEN}✓ No unmarked unsafe code found${NC}"
    ((PASSED_TESTS++))
fi
((TOTAL_TESTS++))

# 6. Panic Detection
echo -e "\n${YELLOW}=== PANIC DETECTION ===${NC}"
echo "Checking for panic! usage..."
if grep -r "panic!" --include="*.rs" btclib/ node/ miner/ wallet/ | grep -v "test" | grep -v "unreachable!"; then
    echo -e "${RED}Found panic! usage in non-test code${NC}"
    ((FAILED_TESTS++))
else
    echo -e "${GREEN}✓ No panic! found in production code${NC}"
    ((PASSED_TESTS++))
fi
((TOTAL_TESTS++))

# 7. Unwrap Detection
echo -e "\n${YELLOW}=== UNWRAP DETECTION ===${NC}"
echo "Checking for unwrap() usage..."
UNWRAP_COUNT=$(grep -r "\.unwrap()" --include="*.rs" btclib/ node/ miner/ wallet/ | grep -v "test" | wc -l)
if [ $UNWRAP_COUNT -gt 0 ]; then
    echo -e "${YELLOW}Warning: Found $UNWRAP_COUNT unwrap() calls in production code${NC}"
    echo "Run 'python scripts/fix_unwraps.py' to see details"
fi

# 8. TODO Detection
echo -e "\n${YELLOW}=== TODO DETECTION ===${NC}"
echo "Checking for TODO comments..."
TODO_COUNT=$(grep -r "TODO" --include="*.rs" btclib/ node/ miner/ wallet/ | wc -l)
if [ $TODO_COUNT -gt 0 ]; then
    echo -e "${YELLOW}Found $TODO_COUNT TODO items remaining${NC}"
    grep -r "TODO" --include="*.rs" btclib/ node/ miner/ wallet/ | head -10
    echo "..."
fi

# 9. Clippy Lints
echo -e "\n${YELLOW}=== CLIPPY ANALYSIS ===${NC}"
run_test_suite "Clippy lints" "cargo clippy --all-targets --all-features -- -D warnings"

# 10. Format Check
echo -e "\n${YELLOW}=== FORMAT CHECK ===${NC}"
run_test_suite "Rust formatting" "cargo fmt --all -- --check"

# Summary
echo -e "\n${YELLOW}=== TEST SUMMARY ===${NC}"
echo "Total test suites: $TOTAL_TESTS"
echo -e "Passed: ${GREEN}$PASSED_TESTS${NC}"
echo -e "Failed: ${RED}$FAILED_TESTS${NC}"

if [ $FAILED_TESTS -eq 0 ]; then
    echo -e "\n${GREEN}🎉 All tests passed! Supernova meets the Satoshi Standard!${NC}"
    exit 0
else
    echo -e "\n${RED}❌ Some tests failed. Please fix before proceeding.${NC}"
    exit 1
fi 
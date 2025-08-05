# Supernova Fuzzing Infrastructure

This directory contains the AFL++ fuzzing infrastructure for the Supernova blockchain, implementing the comprehensive security testing requirements from Section 11.1 of the Security Audit Framework.

## Overview

Our fuzzing infrastructure targets critical security components:

1. **Block Validation** - Tests block parsing and validation logic
2. **Quantum Cryptography** - Tests post-quantum signature implementations
3. **P2P Messages** - Tests network protocol message parsing
4. **Consensus** - Tests fork resolution and chain selection
5. **Transaction Parsing** - Tests transaction deserialization
6. **Difficulty Adjustment** - Tests mining difficulty calculations

## Setup

### Prerequisites

1. Install AFL++:
```bash
# Option 1: Using cargo
cargo install afl

# Option 2: From source (recommended for latest features)
git clone https://github.com/AFLplusplus/AFLplusplus
cd AFLplusplus
make
sudo make install
```

2. Install dependencies:
```bash
# Python for corpus generation
sudo apt-get install python3 screen

# Optional: For coverage analysis
sudo apt-get install lcov
```

### Building Fuzzing Targets

```bash
# Build all fuzzing targets
cd fuzz
cargo afl build --release

# Build specific target
cargo afl build --release --bin fuzz_block_validation
```

## Running Fuzzers

### Quick Start

```bash
# Make scripts executable
chmod +x *.sh

# Run default fuzzer (block validation)
./run_afl.sh

# Run specific fuzzer
./run_afl.sh quantum_crypto

# Run with custom settings
AFL_INSTANCES=8 AFL_TIMEOUT=2000 ./run_afl.sh consensus
```

### Available Targets

- `block_validation` - Block structure and validation rules
- `quantum_crypto` - Dilithium, SPHINCS+, Falcon, Kyber
- `p2p_messages` - Network protocol messages
- `consensus` - Fork resolution and chain selection
- `transaction_parsing` - Transaction deserialization
- `difficulty_adjustment` - Mining difficulty calculations

### Monitoring Progress

```bash
# Check fuzzing status
afl-whatsup findings/block_validation

# View master fuzzer UI
screen -r afl-master

# List all running fuzzers
screen -ls

# Generate performance plots
afl-plot findings/block_validation/fuzzer01 plots/
```

## Analyzing Results

### Finding Crashes

```bash
# Check for crashes
find findings -name "id:*" -path "*/crashes/*" | head -20

# Analyze specific crash
./analyze_crashes.sh block_validation

# Reproduce crash
target/release/fuzz_block_validation < findings/block_validation/fuzzer01/crashes/id:000000,sig:06,src:000000,op:flip1,pos:0
```

### Coverage Analysis

```bash
# Generate coverage report
cargo afl cov -p supernova-fuzz --bin fuzz_block_validation
lcov --capture --directory target/cov --output-file coverage.info
genhtml coverage.info --output-directory coverage-report
```

### Minimizing Test Cases

```bash
# Minimize crash test case
afl-tmin -i findings/block_validation/fuzzer01/crashes/id:000000 \
         -o minimized_crash.bin \
         -- target/release/fuzz_block_validation
```

## Continuous Fuzzing

### GitHub Actions Integration

Add to `.github/workflows/fuzzing.yml`:

```yaml
name: Security Fuzzing

on:
  schedule:
    - cron: '0 0 * * *'  # Daily
  workflow_dispatch:

jobs:
  fuzz:
    runs-on: ubuntu-latest
    strategy:
      matrix:
        target: [block_validation, quantum_crypto, p2p_messages, consensus]
    steps:
      - uses: actions/checkout@v3
      - name: Install AFL++
        run: |
          cargo install afl
      - name: Run Fuzzing
        run: |
          cd fuzz
          timeout 3600 ./run_afl.sh ${{ matrix.target }} || true
      - name: Check for Crashes
        run: |
          if find fuzz/findings -name "id:*" -path "*/crashes/*" | grep -q .; then
            echo "Crashes found!"
            exit 1
          fi
```

### Local Continuous Fuzzing

```bash
# Run fuzzing in background with automatic restart
nohup ./continuous_fuzz.sh > fuzzing.log 2>&1 &

# Run specific target for extended time
timeout 24h ./run_afl.sh quantum_crypto
```

## Best Practices

### Corpus Management

1. **Seed Quality**: Start with high-quality seed inputs
2. **Corpus Minimization**: Regularly minimize corpus
3. **Cross-Pollination**: Share interesting inputs between targets

```bash
# Minimize corpus
afl-cmin -i corpus/block_validation -o corpus_min -- target/release/fuzz_block_validation

# Merge corpuses from multiple fuzzers
mkdir corpus_merged
cp findings/*/fuzzer*/queue/id:* corpus_merged/
```

### Dictionary Creation

Create target-specific dictionaries to improve fuzzing efficiency:

```bash
# Extract strings from binary
strings target/release/supernova-node | grep -E '^[a-zA-Z]{4,}$' | sort -u > dictionaries/strings.dict

# Add protocol-specific tokens
cat >> dictionaries/p2p_messages.dict << EOF
"version"
"verack"
"getblocks"
"\\xf9\\xbe\\xb4\\xd9"  # Network magic
EOF
```

### Performance Tuning

1. **CPU Affinity**: Bind fuzzers to specific cores
```bash
taskset -c 0-3 ./run_afl.sh block_validation
```

2. **Memory Limits**: Adjust based on target
```bash
AFL_MEMORY=512 ./run_afl.sh quantum_crypto
```

3. **Timeout Tuning**: Balance speed vs coverage
```bash
AFL_TIMEOUT=500 ./run_afl.sh p2p_messages  # Fast parsing
AFL_TIMEOUT=5000 ./run_afl.sh consensus    # Complex operations
```

## Security Considerations

### Handling Crashes

1. **Triage**: Not all crashes are security vulnerabilities
2. **Reproduction**: Always verify crashes are reproducible
3. **Root Cause**: Use debugger to find root cause
4. **Fix Verification**: Re-run fuzzer after fixes

### Reporting Issues

When reporting fuzzing findings:

1. Minimize the test case
2. Verify reproducibility
3. Determine security impact
4. Create proof-of-concept if applicable

### Fuzzing Metrics

Track these metrics for security assurance:

- Execution speed (exec/sec)
- Coverage percentage
- Unique crashes found
- Time to first crash
- Stability percentage

## Advanced Topics

### Custom Mutators

For specialized fuzzing needs:

```rust
// In fuzz/mutators/quantum_mutator.rs
pub fn quantum_signature_mutator(data: &mut Vec<u8>) {
    // Custom mutations for quantum signatures
    if data.len() >= 2420 {  // Dilithium signature size
        // Targeted bit flips in signature
        for i in [0, 100, 500, 1000, 2000] {
            if i < data.len() {
                data[i] ^= 1 << (i % 8);
            }
        }
    }
}
```

### Differential Fuzzing

Compare implementations:

```rust
// Compare our quantum crypto with reference implementation
fn differential_fuzz(data: &[u8]) {
    let our_result = our_implementation(data);
    let ref_result = reference_implementation(data);
    assert_eq!(our_result, ref_result);
}
```

### Structure-Aware Fuzzing

Use grammar-based fuzzing for complex structures:

```rust
use arbitrary::{Arbitrary, Unstructured};

#[derive(Arbitrary)]
struct FuzzBlock {
    header: BlockHeader,
    transactions: Vec<Transaction>,
}

fn structure_aware_fuzz(u: &mut Unstructured) {
    let block: FuzzBlock = u.arbitrary().unwrap();
    validate_block(&block.into());
}
```

## Troubleshooting

### Common Issues

1. **Low execution speed**: Check CPU governor, use performance mode
2. **No new paths**: Improve corpus or dictionary
3. **Immediate crashes**: Check target binary with simple inputs
4. **Memory issues**: Adjust AFL_MEMORY setting

### Debug Commands

```bash
# Check AFL++ installation
afl-fuzz --help

# Test fuzzer binary
echo "test" | target/release/fuzz_block_validation

# Check for core dumps
ulimit -c unlimited
```

## References

- [AFL++ Documentation](https://aflplus.plus/)
- [Fuzzing Rust Code](https://rust-fuzz.github.io/book/)
- [Structure-Aware Fuzzing](https://github.com/google/fuzzing/blob/master/docs/structure-aware-fuzzing.md)
- [Supernova Security Audit Framework](../docs/SECURITY_AUDIT.md)

---

For questions or improvements, contact the Supernova Security Team.
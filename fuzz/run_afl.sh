#!/bin/bash
# AFL++ Fuzzing Runner for Supernova
# Based on Security Audit Section 11.1 requirements

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Configuration
AFL_INSTANCES=${AFL_INSTANCES:-4}
AFL_TIMEOUT=${AFL_TIMEOUT:-1000}
AFL_MEMORY=${AFL_MEMORY:-none}
TARGET=${1:-block_validation}

echo -e "${GREEN}Supernova AFL++ Fuzzing Framework${NC}"
echo "======================================"

# Check if AFL++ is installed
if ! command -v afl-fuzz &> /dev/null; then
    echo -e "${RED}AFL++ is not installed!${NC}"
    echo "Please install AFL++ first:"
    echo "  cargo install afl"
    echo "  or"
    echo "  git clone https://github.com/AFLplusplus/AFLplusplus && cd AFLplusplus && make"
    exit 1
fi

# Build the fuzzing target
echo -e "${YELLOW}Building fuzzing target: $TARGET${NC}"
cargo afl build --release --bin fuzz_$TARGET

# Create directories
mkdir -p corpus/$TARGET
mkdir -p findings/$TARGET
mkdir -p dictionaries

# Initialize corpus if empty
if [ -z "$(ls -A corpus/$TARGET)" ]; then
    echo -e "${YELLOW}Initializing corpus for $TARGET${NC}"
    case $TARGET in
        "block_validation")
            # Create sample block for corpus
            python3 -c "
import struct
# Minimal block header (80 bytes)
version = struct.pack('<I', 1)
prev_hash = b'\\x00' * 32
merkle_root = b'\\x00' * 32
timestamp = struct.pack('<Q', 1640000000)
bits = struct.pack('<I', 0x1d00ffff)
nonce = struct.pack('<I', 0)
with open('corpus/$TARGET/seed1.bin', 'wb') as f:
    f.write(version + prev_hash + merkle_root + timestamp + bits + nonce)
"
            ;;
        "quantum_crypto")
            # Create sample crypto data
            echo -n "0123456789abcdef0123456789abcdef" > corpus/$TARGET/seed1.bin
            ;;
        "p2p_messages")
            # Create sample network message
            python3 -c "
import struct
magic = b'\\xf9\\xbe\\xb4\\xd9'  # Bitcoin mainnet magic
command = b'version' + b'\\x00' * 6  # 12 bytes
length = struct.pack('<I', 0)
checksum = b'\\x00' * 4
with open('corpus/$TARGET/seed1.bin', 'wb') as f:
    f.write(magic + command + length + checksum)
"
            ;;
        "consensus")
            # Create sample consensus data
            dd if=/dev/urandom of=corpus/$TARGET/seed1.bin bs=1024 count=1 2>/dev/null
            ;;
        *)
            # Generic seed
            echo "FUZZ" > corpus/$TARGET/seed1.bin
            ;;
    esac
fi

# Create dictionary if it doesn't exist
if [ ! -f dictionaries/$TARGET.dict ]; then
    echo -e "${YELLOW}Creating dictionary for $TARGET${NC}"
    case $TARGET in
        "block_validation")
            cat > dictionaries/$TARGET.dict << EOF
# Block validation dictionary
"\\x00\\x00\\x00\\x00"
"\\xff\\xff\\xff\\xff"
"\\x00\\x00\\x00\\x80"
"version"
"timestamp"
"nonce"
"merkle"
"difficulty"
EOF
            ;;
        "quantum_crypto")
            cat > dictionaries/$TARGET.dict << EOF
# Quantum crypto dictionary
"dilithium"
"sphincs"
"falcon"
"kyber"
"signature"
"publickey"
"privatekey"
"\\x00\\x00\\x00\\x00"
EOF
            ;;
        *)
            touch dictionaries/$TARGET.dict
            ;;
    esac
fi

# Run AFL++ with multiple instances
echo -e "${GREEN}Starting AFL++ with $AFL_INSTANCES instances${NC}"

# Master instance
echo "Starting master fuzzer..."
screen -dmS afl-master \
    afl-fuzz -i corpus/$TARGET -o findings/$TARGET \
    -x dictionaries/$TARGET.dict \
    -t $AFL_TIMEOUT \
    -m $AFL_MEMORY \
    -M fuzzer01 \
    -- target/release/fuzz_$TARGET

sleep 2

# Secondary instances
for i in $(seq 2 $AFL_INSTANCES); do
    echo "Starting secondary fuzzer $i..."
    screen -dmS afl-slave$i \
        afl-fuzz -i corpus/$TARGET -o findings/$TARGET \
        -x dictionaries/$TARGET.dict \
        -t $AFL_TIMEOUT \
        -m $AFL_MEMORY \
        -S fuzzer0$i \
        -- target/release/fuzz_$TARGET
    sleep 1
done

echo -e "${GREEN}Fuzzing started!${NC}"
echo ""
echo "Monitoring commands:"
echo "  screen -r afl-master     # View master fuzzer"
echo "  screen -ls               # List all fuzzers"
echo "  afl-whatsup findings/$TARGET  # Check progress"
echo "  afl-plot findings/$TARGET/fuzzer01 plot_output  # Generate plots"
echo ""
echo "Stop fuzzing:"
echo "  ./stop_fuzzing.sh"
echo ""
echo -e "${YELLOW}Fuzzing in progress...${NC}"

# Optional: Show initial stats
sleep 5
afl-whatsup findings/$TARGET || true
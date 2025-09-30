#!/bin/bash
# Supernova Testnet Launch Script
# This script sets up and launches a Supernova testnet node

set -e  # Exit on error

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${GREEN}===============================================${NC}"
echo -e "${GREEN}    Supernova Testnet Launch Script${NC}"
echo -e "${GREEN}===============================================${NC}"

# Configuration
NETWORK="supernova-testnet"
DATA_DIR="${DATA_DIR:-./testnet-data}"
CONFIG_FILE="${CONFIG_FILE:-./config.toml}"
LOG_FILE="${LOG_FILE:-./testnet.log}"
PID_FILE="${PID_FILE:-./supernova.pid}"

# Check if node binary exists
if [ ! -f "./target/release/supernova-node" ]; then
    echo -e "${YELLOW}Node binary not found. Building...${NC}"
    cargo build --release --all-features
    if [ $? -ne 0 ]; then
        echo -e "${RED}Build failed!${NC}"
        exit 1
    fi
fi

# Create necessary directories
echo -e "${GREEN}Creating directories...${NC}"
mkdir -p "$DATA_DIR"
mkdir -p "$DATA_DIR/blocks"
mkdir -p "$DATA_DIR/state"
mkdir -p "$DATA_DIR/logs"

# Check if config exists
if [ ! -f "$CONFIG_FILE" ]; then
    echo -e "${RED}Configuration file not found: $CONFIG_FILE${NC}"
    exit 1
fi

# Stop any existing node
if [ -f "$PID_FILE" ]; then
    OLD_PID=$(cat "$PID_FILE")
    if ps -p $OLD_PID > /dev/null 2>&1; then
        echo -e "${YELLOW}Stopping existing node (PID: $OLD_PID)...${NC}"
        kill $OLD_PID
        sleep 2
    fi
    rm -f "$PID_FILE"
fi

# Clean up old data if requested
if [ "$1" = "--clean" ]; then
    echo -e "${YELLOW}Cleaning old data...${NC}"
    rm -rf "$DATA_DIR"/*
    rm -f "$LOG_FILE"
fi

# Generate genesis block if not exists
if [ ! -f "$DATA_DIR/genesis.json" ]; then
    echo -e "${GREEN}Generating genesis block...${NC}"
    cat > "$DATA_DIR/genesis.json" << EOF
{
  "version": 1,
  "network": "$NETWORK",
  "genesis_time": "2024-01-01T00:00:00Z",
  "chain_id": "supernova-testnet",
  "initial_supply": "21000000000000000",
  "consensus": {
    "block_time": 600,
    "difficulty": "0x1d00ffff",
    "halving_interval": 210000
  },
  "allocations": [
    {
      "address": "tb1q_testnet_faucet_address",
      "amount": "10500000000000000"
    },
    {
      "address": "tb1q_dev_fund_address",
      "amount": "5250000000000000"
    },
    {
      "address": "tb1q_community_fund_address",
      "amount": "5250000000000000"
    }
  ]
}
EOF
fi

# Start the node
echo -e "${GREEN}Starting Supernova node...${NC}"
echo -e "${GREEN}Network: $NETWORK${NC}"
echo -e "${GREEN}Data directory: $DATA_DIR${NC}"
echo -e "${GREEN}Config file: $CONFIG_FILE${NC}"
echo -e "${GREEN}Log file: $LOG_FILE${NC}"

# Launch node in background
nohup ./target/release/supernova-node \
    --config "$CONFIG_FILE" \
    --debug \
    > "$LOG_FILE" 2>&1 &

NODE_PID=$!
echo $NODE_PID > "$PID_FILE"

# Wait for node to start
echo -e "${YELLOW}Waiting for node to start...${NC}"
sleep 5

# Check if node is running
if ps -p $NODE_PID > /dev/null; then
    echo -e "${GREEN}✅ Node started successfully! (PID: $NODE_PID)${NC}"
    echo -e "${GREEN}===============================================${NC}"
    echo -e "${GREEN}Node Information:${NC}"
    echo -e "  PID: $NODE_PID"
    echo -e "  Network: $NETWORK"
    echo -e "  Data Dir: $DATA_DIR"
    echo -e "  Log File: $LOG_FILE"
    echo -e ""
    echo -e "${YELLOW}Monitor logs with: tail -f $LOG_FILE${NC}"
    echo -e "${YELLOW}Stop node with: kill $NODE_PID${NC}"
    echo -e "${GREEN}===============================================${NC}"
    
    # Show initial log output
    echo -e "\n${GREEN}Initial log output:${NC}"
    tail -n 20 "$LOG_FILE"
else
    echo -e "${RED}❌ Failed to start node!${NC}"
    echo -e "${RED}Check logs for details: $LOG_FILE${NC}"
    tail -n 50 "$LOG_FILE"
    exit 1
fi

# Optional: Start monitoring
if [ "$2" = "--monitor" ]; then
    echo -e "\n${GREEN}Starting log monitoring...${NC}"
    tail -f "$LOG_FILE"
fi

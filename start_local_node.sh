#!/bin/bash

# Script to start a local Supernova node for testing

NODE_NUM=${1:-1}
BASE_PORT=$((30333 + $NODE_NUM))
API_PORT=$((8332 + $NODE_NUM))
DATA_DIR="./data-node-$NODE_NUM"

echo "Starting Supernova Node #$NODE_NUM"
echo "P2P Port: $BASE_PORT"
echo "API Port: $API_PORT"
echo "Data Dir: $DATA_DIR"

# Get the testnet node's peer ID from environment or use default
# Set TESTNET_NODE_IP in your environment to connect to testnet
TESTNET_IP=${TESTNET_NODE_IP:-"testnet.supernovanetwork.xyz"}
TESTNET_NODE="/ip4/${TESTNET_IP}/tcp/30333"

# Build if needed
cargo build --release

# Start the node
./target/release/supernova-node \
  --data-dir "$DATA_DIR" \
  --port $BASE_PORT \
  --api-port $API_PORT \
  --testnet \
  --name "local-node-$NODE_NUM" \
  --bootnodes "$TESTNET_NODE"

# To connect to specific peer ID, add:
# --bootnodes "/ip4/TESTNET_IP/tcp/30333/p2p/PEER_ID_HERE" 
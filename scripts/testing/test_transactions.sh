#!/bin/bash

# Test script for Supernova transactions

echo "=== Supernova Transaction Testing ==="

# Function to create a wallet
create_wallet() {
    local node_port=$1
    echo "Creating wallet on node at port $node_port..."
    curl -X POST http://localhost:$node_port/api/v1/wallet/create \
         -H "Content-Type: application/json" \
         -d '{}'
}

# Function to get balance
get_balance() {
    local node_port=$1
    local address=$2
    echo "Getting balance for $address..."
    curl -s http://localhost:$node_port/api/v1/wallet/balance/$address | jq .
}

# Function to send transaction
send_transaction() {
    local node_port=$1
    local from=$2
    local to=$3
    local amount=$4
    
    echo "Sending $amount NOVA from $from to $to..."
    curl -X POST http://localhost:$node_port/api/v1/transaction/send \
         -H "Content-Type: application/json" \
         -d "{
             \"from\": \"$from\",
             \"to\": \"$to\",
             \"amount\": $amount
         }" | jq .
}

# Function to mine a block
mine_block() {
    local node_port=$1
    echo "Mining a block on node at port $node_port..."
    curl -X POST http://localhost:$node_port/api/v1/mining/mine \
         -H "Content-Type: application/json" \
         -d '{"threads": 1}'
}

# Function to get node info
get_node_info() {
    local node_port=$1
    echo "Node info for port $node_port:"
    curl -s http://localhost:$node_port/api/v1/node/info | jq .
}

# Main testing flow
echo ""
echo "1. Checking node status..."
get_node_info 8332
get_node_info 8333

echo ""
echo "2. Creating wallets..."
WALLET1=$(create_wallet 8332)
WALLET2=$(create_wallet 8333)

echo ""
echo "3. Mining some blocks to get rewards..."
mine_block 8332

echo ""
echo "4. Checking balances..."
# Extract addresses from wallet creation response
# get_balance 8332 "$WALLET1_ADDRESS"

echo ""
echo "Note: Full transaction testing requires:"
echo "- Wallet service endpoints to be implemented"
echo "- Faucet service for initial test tokens"
echo "- Mining functionality enabled" 
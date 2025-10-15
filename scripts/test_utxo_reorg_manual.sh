#!/bin/bash
# Manual UTXO Reorg Testing Script for VPS Nodes
#
# This script validates UTXO reorganization handling on live testnet nodes.
# Requires: 2 VPS nodes (Node-2: 134.209.84.32, Node-3: 207.154.213.122)

set -e

# Configuration
NODE2="http://134.209.84.32:8332"
NODE3="http://207.154.213.122:8332"
API_KEY="test-key-change-in-production"

echo "=================================================="
echo "UTXO Reorg Manual Test Suite"
echo "=================================================="
echo ""

# Test 1: Simple Reorg with Chain Length Difference
echo "=== TEST 1: Simple Reorg (Chain Length) ==="
echo "Scenario: Node-2 mines 3 blocks, Node-3 mines 5 blocks"
echo "Expected: When connected, Node-2 reorgs to Node-3's longer chain"
echo ""

echo "Step 1: Ensure nodes are disconnected..."
# Nodes should start fresh/disconnected

echo "Step 2: Node-2 mines 3 blocks..."
BLOCKS_2=$(curl -s -X POST $NODE2 \
  -H "X-API-Key: $API_KEY" \
  -d '{"jsonrpc":"2.0","method":"generate","params":[3],"id":1}' | jq -r '.result | length')
echo "Node-2 mined $BLOCKS_2 blocks"

NODE2_HEIGHT=$(curl -s -X POST $NODE2 \
  -H "X-API-Key: $API_KEY" \
  -d '{"jsonrpc":"2.0","method":"getblockcount","params":[],"id":1}' | jq '.result')
echo "Node-2 height: $NODE2_HEIGHT"

echo ""
echo "Step 3: Node-3 mines 5 blocks..."
BLOCKS_3=$(curl -s -X POST $NODE3 \
  -H "X-API-Key: $API_KEY" \
  -d '{"jsonrpc":"2.0","method":"generate","params":[5],"id":1}' | jq -r '.result | length')
echo "Node-3 mined $BLOCKS_3 blocks"

NODE3_HEIGHT=$(curl -s -X POST $NODE3 \
  -H "X-API-Key: $API_KEY" \
  -d '{"jsonrpc":"2.0","method":"getblockcount","params":[],"id":1}' | jq '.result')
echo "Node-3 height: $NODE3_HEIGHT"

echo ""
echo "Step 4: Connect nodes (should trigger reorg on Node-2)..."
# User would manually connect nodes with addnode RPC

echo ""
echo "EXPECTED RESULTS:"
echo "- Node-2 logs: 'Reversing transactions from disconnected block' (3 times)"
echo "- Node-2 logs: 'Restored UTXO' messages"
echo "- Node-2 height becomes 5 (or higher if more blocks mined)"
echo "- Node-2 and Node-3 heights match"
echo ""
echo "VERIFICATION COMMANDS:"
echo "On Node-2:"
echo "  grep 'Reversing transactions' /root/supernova/debug_p2p.log"
echo "  grep 'Restored UTXO' /root/supernova/debug_p2p.log | wc -l"
echo ""

# Test 2: UTXO Set Integrity
echo "=== TEST 2: UTXO Set Integrity After Reorg ==="
echo "After Test 1 completes, verify UTXO set consistency"
echo ""

echo "VERIFICATION COMMANDS:"
echo "On Node-2 (after reorg):"
echo "  curl -X POST http://localhost:8332 \\"
echo "    -H 'X-API-Key: $API_KEY' \\"
echo "    -d '{\"jsonrpc\":\"2.0\",\"method\":\"verifyutxoset\",\"params\":[],\"id\":1}'"
echo ""
echo "Expected: UTXO set should be consistent (no errors)"
echo ""

# Test 3: Wallet Balance Accuracy
echo "=== TEST 3: Wallet Balance Verification ==="
echo "Verify wallet can correctly track balance through reorg"
echo ""

echo "Step 1: Record wallet balance before reorg"
echo "Step 2: Trigger reorg (as in Test 1)"
echo "Step 3: Check wallet balance after reorg"
echo ""

echo "VERIFICATION COMMANDS:"
echo "curl -X POST http://node2:8332 \\"
echo "  -H 'X-API-Key: $API_KEY' \\"
echo "  -d '{\"jsonrpc\":\"2.0\",\"method\":\"getbalance\",\"params\":[],\"id\":1}'"
echo ""
echo "Expected: Balance matches UTXOs in new chain tip"
echo ""

# Test 4: Deep Reorg Performance
echo "=== TEST 4: Deep Reorg Performance ==="
echo "Test reorg with 50+ blocks to verify performance"
echo ""

echo "Step 1: Node-2 mines 50 blocks (takes time on testnet)"
echo "Step 2: Node-3 mines 55 blocks separately"
echo "Step 3: Connect nodes and measure reorg time"
echo ""

echo "EXPECTED RESULTS:"
echo "- Reorg completes in <60 seconds"
echo "- All 50 blocks properly disconnected"
echo "- Logs show systematic UTXO restoration"
echo "- No timeouts or errors"
echo ""

# Test 5: Transaction Spending Chain
echo "=== TEST 5: Transaction Spending Chain Reorg ==="
echo "Test reorg when transactions form spending chains"
echo ""

echo "Scenario:"
echo "1. Node-2: Block 1 creates coinbase UTXO"
echo "2. Node-2: Block 2 spends that coinbase"
echo "3. Node-2: Block 3 spends output from block 2"
echo "4. Competing chain arrives with higher work"
echo "5. Reorg should correctly unwind spending chain"
echo ""

echo "VERIFICATION:"
echo "- Original coinbase should be restored (spent in block 2)"
echo "- Block 2 spend transaction removed"
echo "- Block 3 spend transaction removed"
echo "- No orphaned UTXOs"
echo "- Can spend restored coinbase on new chain"
echo ""

# Summary
echo "=================================================="
echo "MANUAL TESTING SUMMARY"
echo "=================================================="
echo ""
echo "Execute these tests on VPS nodes to validate:"
echo "1. Basic reorg functionality (chain length)"
echo "2. UTXO set integrity preserved"
echo "3. Wallet balance accuracy"
echo "4. Performance with deep reorgs"
echo "5. Complex transaction chain handling"
echo ""
echo "All tests should show:"
echo "- ✓ Logs contain 'Reversing transactions' messages"
echo "- ✓ Logs contain 'Restored UTXO' messages"
echo "- ✓ Heights match after sync"
echo "- ✓ Wallet balances correct"
echo "- ✓ UTXO set passes integrity check"
echo ""
echo "=================================================="


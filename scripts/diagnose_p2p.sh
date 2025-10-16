#!/bin/bash
# P2P Diagnostic Script for VPS Testing

echo "=== Supernova P2P Diagnostics ==="
echo "Node: $(hostname)"
echo "Date: $(date)"
echo ""

# Build and deploy
echo "Building latest binary..."
cargo build --release

# Start node with enhanced logging
echo "Starting node with P2P diagnostics..."
RUST_LOG=info,libp2p=debug,libp2p_identify=debug,libp2p_swarm=debug \
  ./target/release/supernova-node > p2p_debug.log 2>&1 &

NODE_PID=$!
echo "Node started (PID: $NODE_PID)"
echo "Logging to: p2p_debug.log"
echo ""
echo "Let node run for 2 minutes, then check logs..."
echo ""
echo "To monitor: tail -f p2p_debug.log"
echo "To analyze: grep 'CONNECTION\|IDENTIFY\|GOSSIPSUB' p2p_debug.log"
echo ""
echo "When ready to connect to peer:"
echo "  curl -X POST http://localhost:8332 -H 'X-API-Key: test-key-change-in-production' \\"
echo "    -d '{\"jsonrpc\":\"2.0\",\"method\":\"addnode\",\"params\":[\"PEER_ADDRESS:8333\",\"add\"],\"id\":1}'"


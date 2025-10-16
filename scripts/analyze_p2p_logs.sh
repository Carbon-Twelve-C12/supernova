#!/bin/bash
# Analyze P2P diagnostic logs

echo "=== P2P Connection Analysis ==="
echo ""

echo "CONNECTION ESTABLISHED events:"
grep "CONNECTION ESTABLISHED" p2p_debug.log | wc -l

echo ""
echo "IDENTIFY RECEIVED events:"
grep "IDENTIFY RECEIVED" p2p_debug.log | wc -l

echo ""
echo "CONNECTION CLOSED events:"
grep "CONNECTION CLOSED" p2p_debug.log | wc -l

echo ""
echo "=== Recent Connections (Last 10) ==="
grep "CONNECTION ESTABLISHED\|CONNECTION CLOSED" p2p_debug.log | tail -10

echo ""
echo "=== Identify Protocol Exchange ==="
grep "IDENTIFY RECEIVED" p2p_debug.log | tail -5

echo ""
echo "=== Supported Protocols ==="
grep -A 20 "Supported Protocols" p2p_debug.log | tail -25

echo ""
echo "=== Gossipsub Mesh Status ==="
grep -i "adding peer.*to.*mesh\|removing peer.*from mesh\|mesh.*blocks" p2p_debug.log | tail -10

echo ""
echo "=== Gossipsub Publish Attempts ==="
grep -i "publish\|announcing.*block" p2p_debug.log | tail -10

echo ""
echo "=== Protocol Mismatches/Errors ==="
grep -i "protocol.*not.*supported\|missing.*protocol\|incompatible" p2p_debug.log

echo ""
echo "=== Recent Errors ==="
grep -iE "error|critical|fail" p2p_debug.log | grep -v "ValidationMode\|no error" | tail -15

echo ""
echo "=== Connection Duration Analysis ==="
echo "Check if connections are stable or closing quickly:"
grep "CONNECTION ESTABLISHED" p2p_debug.log | head -1
echo "First connection at: ^"
grep "CONNECTION CLOSED" p2p_debug.log | head -1
echo "First closure at: ^"

echo ""
echo "=== Summary ==="
echo "Total connections: $(grep -c 'CONNECTION ESTABLISHED' p2p_debug.log)"
echo "Total closures: $(grep -c 'CONNECTION CLOSED' p2p_debug.log)"
echo "Active connections: $(($(grep -c 'CONNECTION ESTABLISHED' p2p_debug.log) - $(grep -c 'CONNECTION CLOSED' p2p_debug.log)))"


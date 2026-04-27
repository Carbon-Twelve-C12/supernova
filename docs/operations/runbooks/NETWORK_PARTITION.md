# Runbook: Network Partition Recovery

## Alert
`ChainTipDivergence` or `ForkDetected` - Multiple competing chain tips

## Severity
**CRITICAL (P0)** - Potential chain split

## Description
Network partition occurs when groups of nodes become isolated and follow different chain tips. This can lead to double-spend attacks or consensus failures.

## RTO/RPO
- **RTO (Recovery Time Objective):** 30 minutes to identify, 2 hours to resolve
- **RPO (Recovery Point Objective):** Canonical chain (most accumulated work)

## Symptoms
- Alert: `ChainTipDivergence` or `ForkDetected` firing
- Different block hashes reported at same height
- Peer disconnections or reduced peer count
- Transaction confirmations varying between nodes

## Immediate Actions

### 1. Assess Partition Scope (5 minutes)

```bash
# Check local chain tips
supernova-cli getchaintips

# Get current best block
supernova-cli getbestblockhash
supernova-cli getblock $(supernova-cli getbestblockhash) | jq '.height, .hash'

# Check peer view
supernova-cli getpeerinfo | jq '.[] | {addr: .addr, height: .synced_blocks, hash: .synced_headers_hash}'
```

### 2. Identify Partition Boundaries (10 minutes)

```bash
# Query multiple known good nodes
for node in node1.supernova.io node2.supernova.io node3.supernova.io; do
  echo "=== $node ==="
  curl -s "http://$node:8332/api/v1/blockchain/info" | jq '.best_block_hash, .height'
done

# Check geographic distribution
supernova-cli getpeerinfo | jq '.[] | {addr: .addr, country: .geo_info.country}'
```

### 3. Determine Canonical Chain (5 minutes)

The canonical chain is determined by:
1. Most accumulated proof-of-work
2. Environmental bonuses (if applicable)
3. Checkpoint compliance

```bash
# Get chain work for each tip
supernova-cli getchaintips | jq '.[] | {hash: .hash, height: .height, chainwork: .chainwork}'

# The tip with highest chainwork is canonical
# If tied, the one seen first by majority of nodes wins
```

## Resolution Procedures

### Scenario A: Temporary Network Issue

```bash
# 1. Wait for automatic resolution (nodes will converge)
# Monitor for 10-15 minutes

# 2. If not resolving, manually add peer connections
supernova-cli addnode "known-good-node.supernova.io" "onetry"

# 3. Force sync with canonical chain
supernova-cli invalidateblock <non-canonical-tip-hash>
supernova-cli reconsiderblock <canonical-tip-hash>
```

### Scenario B: Intentional Attack (Eclipse Attack)

```bash
# 1. Disconnect all suspicious peers
supernova-cli disconnectnode "<suspicious-peer>"

# 2. Connect to trusted seed nodes only
supernova-cli clearbanned
for seed in seed1.supernova.io seed2.supernova.io; do
  supernova-cli addnode "$seed" "add"
done

# 3. Enable eclipse attack protection
supernova-cli setnetworkparam eclipse_protection true

# 4. If needed, add emergency checkpoint
supernova-cli addcheckpoint <canonical-block-hash> <height>
```

### Scenario C: Software Bug Causing Divergence

```bash
# 1. Identify the block where divergence started
supernova-cli getblock <divergent-block-hash> true

# 2. Check for validation differences
journalctl -u supernova-node | grep -i "validation\|reject"

# 3. If bug confirmed:
# - Halt affected functionality
# - Deploy emergency patch
# - Coordinate network-wide upgrade

# 4. DO NOT attempt manual chain manipulation without protocol team approval
```

## Coordination with Node Operators

### Emergency Communication

```
[URGENT] Network Partition Detected

All node operators: Please verify your node is on the canonical chain.

Canonical chain:
- Block Height: [HEIGHT]
- Block Hash: [HASH]
- Chain Work: [WORK]

If your node shows a different best block hash:
1. Run: supernova-cli invalidateblock <your-tip-hash>
2. Run: supernova-cli reconsiderblock [CANONICAL-HASH]
3. Verify sync: supernova-cli getbestblockhash

Contact: #node-operators on Discord
```

### Verify Node Operator Convergence

```bash
# Create a checklist of major node operators
# Verify each has converged to canonical chain
# Document any holdouts and coordinate directly
```

## Recovery Verification

```bash
# 1. Single chain tip only
supernova-cli getchaintips | jq 'length'
# Should return 1

# 2. All peers on same chain
supernova-cli getpeerinfo | jq '.[].synced_headers_hash' | sort | uniq -c
# Should show single hash with count = peer count

# 3. Block production resumed normally
supernova-cli getblockcount
# Wait 10 minutes, should increase

# 4. Transaction confirmations working
supernova-cli sendrawtransaction <test-tx> && echo "TX propagating normally"
```

## Prevention Measures

After resolution, implement:

1. **Increase peer diversity:**
   ```bash
   # Add geographic diversity
   supernova-cli setnetworkparam min_peer_countries 5
   ```

2. **Enable checkpoint protection:**
   ```bash
   # Add recent checkpoint
   supernova-cli addcheckpoint <recent-block-hash> <height>
   ```

3. **Review network monitoring:**
   - Ensure `ChainTipDivergence` alert is tuned correctly
   - Add alerts for peer count drops

## Escalation Path

1. **0-10 min:** On-call engineer assesses scope
2. **10-20 min:** Protocol team engaged
3. **20+ min:** Incident Commander declares SEV-1
4. **If attack confirmed:** Security team lead engaged

## Related Documentation

- [Consensus Halt Runbook](./CONSENSUS_HALT.md)
- [Emergency Hard Fork Process](../INCIDENT_RESPONSE.md#emergency-hard-fork-process)
- [Incident Response Plan](../INCIDENT_RESPONSE.md)

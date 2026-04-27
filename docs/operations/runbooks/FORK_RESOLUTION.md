# Runbook: Fork Resolution Procedures

## Alert
`ForkDetected` - Multiple competing chain tips with significant work

## Severity
**HIGH (P1)** to **CRITICAL (P0)** depending on fork depth

## Description
A chain fork occurs when miners produce competing blocks at the same height. Shallow forks (1-2 blocks) are normal; deep forks (>6 blocks) indicate potential attacks or consensus bugs.

## RTO/RPO
- **RTO:** 30 minutes for assessment, 2 hours for resolution
- **RPO:** Canonical chain (highest accumulated work + environmental bonuses)

## Fork Classification

| Depth | Classification | Typical Cause | Response |
|-------|---------------|---------------|----------|
| 1-2 blocks | Normal | Network latency | Monitor, auto-resolves |
| 3-5 blocks | Concerning | High latency or minor issue | Investigate |
| 6-10 blocks | Serious | Potential attack or bug | Active response |
| >10 blocks | Critical | Active attack or major bug | Emergency response |

## Immediate Assessment

### 1. Identify Fork Details (5 minutes)

```bash
# Get all chain tips
supernova-cli getchaintips

# Output interpretation:
# status: "active" = current best chain
# status: "valid-fork" = valid competing chain
# status: "valid-headers" = headers only, not fully validated
# status: "invalid" = rejected chain

# Get fork details
supernova-cli getchaintips | jq '.[] | select(.status == "valid-fork")'
```

### 2. Calculate Fork Metrics (5 minutes)

```bash
# Get chainwork for each tip
ACTIVE_TIP=$(supernova-cli getbestblockhash)
FORK_TIP="<fork-tip-hash>"

supernova-cli getblock $ACTIVE_TIP | jq '.chainwork'
supernova-cli getblock $FORK_TIP | jq '.chainwork'

# Find common ancestor
supernova-cli getblockheader $ACTIVE_TIP | jq '.height'
supernova-cli getblockheader $FORK_TIP | jq '.height'

# Calculate fork depth
ACTIVE_HEIGHT=$(supernova-cli getblockheader $ACTIVE_TIP | jq '.height')
FORK_HEIGHT=$(supernova-cli getblockheader $FORK_TIP | jq '.height')
```

### 3. Check for Environmental Bonus Differences

```bash
# Supernova uses environmental bonuses in fork resolution
supernova-cli getblock $ACTIVE_TIP | jq '.environmental_bonus'
supernova-cli getblock $FORK_TIP | jq '.environmental_bonus'

# Higher environmental bonus adds to effective chainwork
```

## Resolution Procedures

### Scenario A: Normal Fork (1-2 blocks)

```bash
# 1. Monitor - usually auto-resolves
watch -n 10 'supernova-cli getchaintips | jq "length"'

# 2. If persists >30 minutes, check peer diversity
supernova-cli getpeerinfo | jq '.[].synced_blocks'

# 3. May need to add peers
supernova-cli addnode "seed.supernova.io" "onetry"
```

### Scenario B: Deep Fork (3-10 blocks)

```bash
# 1. Identify which chain has more work
supernova-cli getchaintips | jq 'sort_by(.chainwork) | reverse | .[0]'

# 2. Check transaction differences
# Some transactions may be in one chain but not other
supernova-cli getblock $ACTIVE_TIP true | jq '.tx'
supernova-cli getblock $FORK_TIP true | jq '.tx'

# 3. Verify no double-spends
supernova-cli getrawmempool | head -20

# 4. If your node is on wrong chain, switch
supernova-cli invalidateblock $WRONG_TIP
supernova-cli reconsiderblock $CORRECT_TIP
```

### Scenario C: Attack Fork (>10 blocks)

```bash
# 1. IMMEDIATELY notify incident response team
# This may be a 51% attack

# 2. Preserve evidence
supernova-cli getchaintips > /tmp/fork_evidence_$(date +%s).json
for tip in $(supernova-cli getchaintips | jq -r '.[].hash'); do
    supernova-cli getblock $tip true >> /tmp/fork_blocks_$(date +%s).json
done

# 3. Add emergency checkpoint if consensus reached
# This requires coordination with protocol team
supernova-cli addcheckpoint <canonical-hash> <height>

# 4. Contact exchanges to pause deposits
# Require more confirmations for deposits

# 5. Coordinate with mining pools
# Verify hashrate distribution
```

## Transaction Recovery

After fork resolution, some transactions may need recovery:

```bash
# 1. Get transactions from orphaned chain
ORPHAN_BLOCK="<orphaned-block-hash>"
supernova-cli getblock $ORPHAN_BLOCK true | jq '.tx[]'

# 2. Check which are in mempool
for txid in $(supernova-cli getblock $ORPHAN_BLOCK | jq -r '.tx[]'); do
    supernova-cli getmempoolentry $txid 2>/dev/null && echo "$txid in mempool"
done

# 3. Rebroadcast valid transactions not in canonical chain
supernova-cli sendrawtransaction <raw-tx>
```

## Monitoring During Resolution

```bash
# Watch chain tips count
watch -n 5 'supernova-cli getchaintips | jq "length"'

# Monitor for new forks
watch -n 5 'supernova-cli getchaintips | jq ".[] | select(.status == \"valid-fork\")"'

# Track hashrate
watch -n 30 'supernova-cli getmininginfo | jq ".networkhashps"'
```

## Prevention Measures

After resolution:

1. **Add checkpoint:**
   ```bash
   # Add recent checkpoint to prevent future deep reorgs
   supernova-cli addcheckpoint $(supernova-cli getbestblockhash) $(supernova-cli getblockcount)
   ```

2. **Increase confirmations:**
   ```bash
   # Recommend increased confirmation requirements
   # Update exchange integrations, user guidance
   ```

3. **Review hashrate distribution:**
   ```bash
   # Monitor for hashrate concentration
   supernova-cli getmininginfo | jq '.pooledtx, .networkhashps'
   ```

## Escalation

| Fork Depth | Escalation Level |
|------------|------------------|
| 1-2 blocks | No escalation needed |
| 3-5 blocks | Notify on-call, monitor closely |
| 6-10 blocks | Protocol team + Infrastructure |
| >10 blocks | Full incident response, SEV-1 |

## Communication

For deep forks (>6 blocks):

```
[ALERT] Chain Fork Detected

A chain fork has been detected on the Supernova network.

Fork Depth: [X] blocks
Status: [Investigating/Resolving]
Transaction Impact: [None/Pending transactions may be affected]

For exchanges: Consider requiring additional confirmations.
For users: Wait for additional confirmations before considering transactions final.

Updates: status.supernova.io
```

## Related Documentation

- [Network Partition Runbook](./NETWORK_PARTITION.md)
- [Consensus Halt Runbook](./CONSENSUS_HALT.md)
- [Incident Response Plan](../INCIDENT_RESPONSE.md)

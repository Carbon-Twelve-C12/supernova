# Runbook: Consensus Halt Recovery

## Alert
`ConsensusHalt` - No blocks produced for 30+ minutes

## Severity
**CRITICAL (P0)** - Requires immediate response

## Description
The network has stopped producing blocks. This indicates a consensus failure that prevents miners from creating valid blocks.

## RTO/RPO
- **RTO (Recovery Time Objective):** 1 hour
- **RPO (Recovery Point Objective):** Last valid block

## Symptoms
- Prometheus alert: `ConsensusHalt` firing
- `supernova_last_block_timestamp` shows >30 minutes old
- No new blocks appearing in logs
- Mempool growing but not clearing

## Immediate Actions

### 1. Verify the Halt (2 minutes)

```bash
# Check current block height and timestamp
supernova-cli getblockcount
supernova-cli getbestblockhash
supernova-cli getblock $(supernova-cli getbestblockhash) | jq '.time'

# Compare with current time
date +%s

# Check peer status
supernova-cli getpeerinfo | jq 'length'
```

### 2. Check Network Connectivity (3 minutes)

```bash
# Verify peers are connected
supernova-cli getnetworkinfo

# Check if peers have newer blocks
supernova-cli getpeerinfo | jq '.[].synced_blocks'

# Check for network partition
supernova-cli getchaintips
```

### 3. Identify Root Cause (5 minutes)

**Check for common causes:**

```bash
# A. Mining issues - check if mining is enabled
supernova-cli getmininginfo

# B. Consensus rule failure - check logs for rejection
journalctl -u supernova-node --since "30 minutes ago" | grep -i "reject\|invalid\|error"

# C. Fork detection
supernova-cli getchaintips | jq '.[] | select(.status != "active")'

# D. Environmental oracle issues (if applicable)
supernova-cli getenvironmentalmetrics
```

## Root Cause Resolution

### Cause A: Mining Stopped

```bash
# Restart mining
supernova-cli setgenerate true

# Verify mining resumed
supernova-cli getmininginfo | jq '.generate'
```

### Cause B: Consensus Bug

1. **DO NOT attempt fixes without review**
2. Gather debug information:
   ```bash
   supernova-cli getblocktemplate | jq '.previous_block'
   journalctl -u supernova-node --since "1 hour ago" > /tmp/consensus_debug.log
   ```
3. Escalate to protocol team with logs
4. If a known bug, deploy hotfix per emergency release process

### Cause C: Network Partition

```bash
# Check for multiple chain tips
supernova-cli getchaintips

# If partition detected, identify majority chain
# Contact major node operators via emergency channels

# Add checkpoints if needed
supernova-cli addcheckpoint <blockhash> <height>
```

### Cause D: Environmental Oracle Failure

```bash
# Check oracle status
supernova-cli getoraclestatus

# If oracle is down, verify fallback is active
supernova-cli getmininginfo | jq '.environmental_bonus'

# Mining should continue without environmental bonus
```

## Recovery Verification

```bash
# 1. Confirm new blocks being produced
watch -n 5 'supernova-cli getblockcount'

# 2. Check mempool is clearing
supernova-cli getmempoolinfo | jq '.size'

# 3. Verify peer sync
supernova-cli getpeerinfo | jq '.[0].synced_blocks'

# 4. Check no more chain tips divergence
supernova-cli getchaintips | jq 'length'
```

## Escalation Path

1. **0-15 min:** On-call engineer investigates
2. **15-30 min:** Escalate to protocol team lead
3. **30+ min:** Incident Commander declares SEV-1
4. **1+ hour:** Consider emergency hard fork procedures

## Communication Template

```
[ALERT] Supernova Network - Block Production Issue

Status: Investigating block production delay
Impact: New transactions not confirming
Start Time: [UTC TIME]
Current Block Height: [HEIGHT]

Updates will be posted every 15 minutes.

Follow @SupernovaStatus for updates.
```

## Post-Incident

1. Create incident ticket with timeline
2. Gather all logs and metrics
3. Schedule post-mortem within 48 hours
4. Update this runbook with lessons learned

## Related Documentation

- [Incident Response Plan](../INCIDENT_RESPONSE.md)
- [Emergency Hard Fork Process](../INCIDENT_RESPONSE.md#emergency-hard-fork-process)
- [Network Partition Runbook](./NETWORK_PARTITION.md)

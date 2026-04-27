# Runbook: Quantum Security Emergency Response

## Alert
`QuantumCanaryTriggered` - Potential quantum computing threat detected

## Severity
**CRITICAL (P0)** - Cryptographic security at risk

## Description
The quantum canary system has detected potential compromise of classical cryptographic signatures. This could indicate:
1. A cryptographically relevant quantum computer has emerged
2. A novel classical attack on current signature schemes
3. False positive requiring investigation

## RTO/RPO
- **RTO (Recovery Time Objective):** Begin key migration within 1 hour
- **RPO (Recovery Point Objective):** Protect all funds not already compromised

## Symptoms
- Alert: `QuantumCanaryTriggered` firing
- `supernova_quantum_canary_status == 0`
- Canary UTXOs showing unexpected spends
- Classical signature verification anomalies

## CRITICAL: Initial Assessment (15 minutes)

### 1. Verify Canary Status

```bash
# Check canary system status
supernova-cli getquantumcanarystatus

# Expected output if triggered:
# {
#   "status": "triggered",
#   "triggered_at": "2024-...",
#   "canary_utxos_compromised": 3,
#   "threat_assessment": "high"
# }

# Check for false positive indicators
supernova-cli getquantumcanarydetails
```

### 2. Assess Threat Reality

**True Positive Indicators:**
- Multiple independent canary UTXOs spent
- Spends signed with valid classical signatures
- No known private key exposure

**False Positive Indicators:**
- Single canary affected (possible key leak)
- Known vulnerability being tested
- Canary system malfunction

### 3. Determine Response Level

| Threat Level | Action |
|--------------|--------|
| **False Positive** | Investigate source, refresh canaries |
| **Possible Threat** | Advisory to high-value holders |
| **Confirmed Threat** | Emergency network-wide migration |

## Response Procedures

### Level 1: False Positive Response

```bash
# 1. Investigate specific canary
supernova-cli getquantumcanaryinfo <canary_id>

# 2. Check for operational issues
journalctl -u supernova-node | grep -i "canary\|quantum"

# 3. If key leak suspected, generate new canaries
supernova-cli refreshquantumcanaries --count 10

# 4. Document and close as false positive
```

### Level 2: Advisory Response

```bash
# 1. Issue security advisory to high-value holders
# Use pre-drafted communication templates

# 2. Recommend voluntary migration
supernova-cli estimatemigrationcost <address>

# 3. Increase canary monitoring sensitivity
supernova-cli setquantumcanaryconfig sensitivity=high

# 4. Prepare for potential escalation
```

### Level 3: Emergency Migration Response

#### Phase 1: Immediate Actions (0-1 hour)

```bash
# 1. Activate emergency response team
# Contact list in secure internal documentation

# 2. Preserve all evidence
supernova-cli exportquantumcanarylogs > /secure/canary_evidence.json

# 3. Begin emergency key rotation for infrastructure
supernova-cli rotatekeys --quantum-resistant --emergency
```

#### Phase 2: Network Protection (1-4 hours)

```bash
# 1. Enable quantum migration mode
supernova-cli setnetworkmode quantum_migration

# 2. Prioritize quantum-resistant transactions
supernova-cli setmempoolpriority quantum_resistant=high

# 3. Issue emergency checkpoint
supernova-cli addcheckpoint $(supernova-cli getbestblockhash) $(supernova-cli getblockcount)

# 4. Coordinate with exchanges
# - Request deposit/withdrawal pause
# - Verify their migration readiness
```

#### Phase 3: User Migration (4-48 hours)

```bash
# Migration tools for users
supernova-cli migratetoquantum --wallet <wallet_file>

# Bulk migration for custodians
supernova-cli batchmigrate --input addresses.txt --quantum-resistant
```

### Emergency Hard Fork (If Required)

If the threat is severe and migration pace insufficient:

```bash
# 1. Prepare emergency fork
# - Invalidate classical-only signatures after block height X
# - This is a BREAKING CHANGE requiring coordination

# 2. Deploy emergency release
# Follow emergency hard fork process in INCIDENT_RESPONSE.md

# 3. Coordinate activation
# Minimum 24-hour notice to exchanges if possible
```

## Key Migration Guide

### For Individual Users

```bash
# 1. Check current key type
supernova-cli getaddressinfo <address> | jq '.key_type'

# 2. Generate quantum-resistant address
supernova-cli getnewaddress --quantum-resistant

# 3. Migrate funds
supernova-cli sendtoaddress <new_quantum_address> <amount> --priority high
```

### For Custodians/Exchanges

```bash
# 1. Generate quantum-resistant hot wallet
supernova-cli createwallet "quantum_hot" --quantum-resistant

# 2. Batch migrate cold storage
supernova-cli signrawtransaction <migration_tx> --keypool quantum

# 3. Update deposit addresses
supernova-cli getnewaddress --quantum-resistant --label "user_deposits"
```

## Monitoring During Emergency

```bash
# Real-time migration progress
watch -n 60 'supernova-cli getquantummigrationstats'

# Expected output:
# {
#   "total_utxos": 12345678,
#   "migrated_utxos": 1234567,
#   "migration_percentage": 10.0,
#   "value_migrated_btc": 123456.78,
#   "estimated_completion": "2024-..."
# }

# High-value address tracking
supernova-cli getunmigratedlargebalances --min-value 1000
```

## Communication Templates

### Initial Alert (Internal)

```
[QUANTUM ALERT - INTERNAL]

Quantum canary triggered at [TIME UTC]
Canary ID: [ID]
Initial Assessment: [Investigating/Possible/Confirmed]

DO NOT discuss externally until cleared.
Assemble at #quantum-emergency
```

### Public Advisory (If Confirmed)

```
SECURITY ADVISORY: Quantum Migration Recommended

The Supernova security team has detected indicators suggesting
advances in quantum computing capability.

As a precaution, we recommend users migrate to quantum-resistant
addresses. Your funds remain secure, but migration provides
additional future protection.

Migration Guide: https://docs.supernova.io/quantum-migration
Status Updates: @SupernovaStatus

This is a PRECAUTIONARY measure. No funds have been compromised.
```

### Emergency Alert (If Active Threat)

```
URGENT SECURITY ALERT: Immediate Action Required

A credible quantum computing threat has been identified.
All users should migrate to quantum-resistant addresses immediately.

Steps:
1. Update to latest Supernova wallet
2. Generate new quantum-resistant address
3. Transfer all funds to new address

DO NOT send funds to classical (non-quantum) addresses.

Emergency Support: security@supernova.io
Status: status.supernova.io
```

## Recovery Verification

```bash
# 1. Canary system stabilized
supernova-cli getquantumcanarystatus | jq '.status'
# Expected: "monitoring" or "refreshed"

# 2. Migration progress satisfactory
supernova-cli getquantummigrationstats | jq '.migration_percentage'
# Target: >80% within 48 hours for confirmed threat

# 3. No new classical signature anomalies
supernova-cli getclassicalsigstats --period 24h
# Expected: anomaly_count = 0

# 4. Network operating normally
supernova-cli getnetworkinfo | jq '.quantum_mode'
# Expected: "normal" or "migration_complete"
```

## Post-Incident

1. **Full forensic analysis** of canary compromise
2. **Security advisory update** to community
3. **Refresh all canary UTXOs** with new key types
4. **Review and update** detection thresholds
5. **Coordinate with** academic/security community

## Related Documentation

- [Quantum Security Architecture](../../architecture/QUANTUM_SECURITY.md)
- [Key Migration Guide](../../user-guides/QUANTUM_MIGRATION.md)
- [Incident Response Plan](../INCIDENT_RESPONSE.md)
- [Emergency Hard Fork Process](../INCIDENT_RESPONSE.md#emergency-hard-fork-process)

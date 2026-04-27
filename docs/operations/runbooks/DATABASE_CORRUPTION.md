# Runbook: Database Corruption Recovery

## Alert
`UTXOSetCorruption` or `DatabaseCapacityCritical`

## Severity
**CRITICAL (P0)** - Data integrity at risk

## Description
Database corruption can occur due to hardware failures, improper shutdowns, or software bugs. This runbook covers identification and recovery procedures.

## RTO/RPO
- **RTO (Recovery Time Objective):** 4 hours
- **RPO (Recovery Point Objective):** Last verified backup (max 24 hours)

## Symptoms
- Alert: `UTXOSetCorruption` or database errors
- Node fails to start with database errors
- Block validation failures on known-good blocks
- Inconsistent balance queries
- `repair check` failures

## Immediate Actions

### 1. Stop the Node (1 minute)

```bash
# Graceful shutdown to prevent further corruption
systemctl stop supernova-node

# Verify stopped
systemctl status supernova-node
```

### 2. Preserve Evidence (5 minutes)

```bash
# Create a snapshot before any repair attempts
TIMESTAMP=$(date +%Y%m%d_%H%M%S)
cp -r /var/lib/supernova /var/lib/supernova.corrupted.$TIMESTAMP

# Capture system state
dmesg | tail -100 > /tmp/dmesg_$TIMESTAMP.log
journalctl -u supernova-node --since "1 hour ago" > /tmp/node_$TIMESTAMP.log
```

### 3. Assess Corruption Scope (10 minutes)

```bash
# Run integrity check (offline)
supernova-cli repair check --level full --offline

# Check disk health
smartctl -a /dev/sda | grep -E "Reallocated|Pending|Uncorrectable"

# Check filesystem
fsck -n /dev/sda1  # Read-only check
```

## Recovery Procedures

### Option A: Automatic Repair (Preferred)

```bash
# Attempt automatic repair
supernova-cli repair auto

# If successful, verify
supernova-cli repair check --level deep

# Restart node
systemctl start supernova-node
```

### Option B: Component-Specific Repair

#### UTXO Set Corruption

```bash
# Rebuild UTXO set from blocks
supernova-cli repair rebuild-utxo --progress

# This replays all blocks to reconstruct UTXO set
# Duration: 1-4 hours depending on chain height
```

#### Index Corruption

```bash
# Rebuild indexes only
supernova-cli repair rebuild-indexes

# Duration: 10-30 minutes
```

#### Block Data Corruption

```bash
# Attempt to repair from peers
supernova-cli repair repair-blocks --from-peers

# If that fails, identify corrupted range
supernova-cli repair check --verbose | grep -i corrupt

# Remove corrupted blocks and re-sync
supernova-cli repair revert-checkpoint --height <last-good-height>
```

### Option C: Restore from Backup

If repair fails, restore from backup:

```bash
# 1. Verify backup integrity
supernova-cli backup verify /backup/supernova/latest

# 2. Stop node if running
systemctl stop supernova-node

# 3. Clear corrupted data
rm -rf /var/lib/supernova/*

# 4. Restore backup
supernova-cli backup restore --source /backup/supernova/latest

# 5. Verify restoration
supernova-cli repair check --level standard

# 6. Start node and sync remaining blocks
systemctl start supernova-node
```

### Option D: Full Resync (Last Resort)

If no valid backup exists:

```bash
# 1. Preserve wallet data if possible
cp /var/lib/supernova/wallet.dat /tmp/wallet.dat.backup

# 2. Clear all data
rm -rf /var/lib/supernova/*

# 3. Restore wallet
mkdir -p /var/lib/supernova
cp /tmp/wallet.dat.backup /var/lib/supernova/wallet.dat

# 4. Start fresh sync
systemctl start supernova-node

# 5. Monitor sync progress
watch 'supernova-cli getblockchaininfo | jq ".blocks, .headers, .verificationprogress"'

# Full sync duration: 12-48 hours depending on hardware
```

## Hardware Investigation

If corruption is recurring:

```bash
# Check disk SMART status
smartctl -H /dev/sda

# Check for memory errors
memtest86+  # Boot from memtest USB

# Check filesystem consistency
umount /var/lib/supernova
fsck.ext4 -f /dev/sda1

# Review kernel logs for I/O errors
dmesg | grep -i "error\|fail\|i/o"
```

## Recovery Verification

```bash
# 1. Full integrity check passes
supernova-cli repair check --level deep
# Expected: All checks PASSED

# 2. Node syncs successfully
supernova-cli getblockchaininfo | jq '.verificationprogress'
# Expected: 0.9999... (fully synced)

# 3. Transactions validate correctly
supernova-cli gettxoutsetinfo
# Compare UTXO hash with known-good nodes

# 4. No errors in logs for 1 hour
journalctl -u supernova-node --since "1 hour ago" | grep -i error
# Expected: No critical errors
```

## Prevention Measures

1. **Enable checksums:**
   ```toml
   # In supernova.toml
   [storage]
   enable_checksums = true
   verify_on_read = true
   ```

2. **Regular backups:**
   ```bash
   # Daily incremental, weekly full
   0 2 * * * supernova-cli backup create --mode incremental
   0 3 * * 0 supernova-cli backup create --mode full
   ```

3. **Hardware monitoring:**
   - Set up SMART monitoring alerts
   - Use ECC RAM if available
   - Ensure proper UPS protection

4. **Regular integrity checks:**
   ```bash
   # Weekly full check
   0 4 * * 0 supernova-cli repair check --level full
   ```

## Escalation Path

1. **0-30 min:** On-call engineer attempts auto repair
2. **30-60 min:** If repair fails, begin backup restore
3. **1-2 hours:** If backup restore fails, escalate to infrastructure team
4. **2+ hours:** Consider full resync as last resort

## Communication Template

```
[ALERT] Supernova Node - Database Recovery in Progress

Status: Recovering from database corruption
Impact: Node temporarily offline
Start Time: [UTC TIME]

Recovery Method: [Auto-repair / Backup restore / Resync]
Estimated Completion: [TIME]

User Impact:
- API requests may fail
- Transactions still propagating via other nodes

Updates every 30 minutes.
```

## Related Documentation

- [Backup and Restore Procedures](../BACKUP_RESTORE.md)
- [Incident Response Plan](../INCIDENT_RESPONSE.md)
- [Hardware Requirements](../../deployment/HARDWARE.md)

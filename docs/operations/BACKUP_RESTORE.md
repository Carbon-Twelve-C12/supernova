# Supernova Backup and Restore Procedures

This document outlines the backup, restore, and recovery procedures for Supernova blockchain nodes.

## Overview

Supernova provides multiple layers of data protection:

1. **Write-Ahead Log (WAL)**: Transaction journaling for crash recovery
2. **Checksums**: CRC32 on WAL entries, SHA256 on database files
3. **Integrity Verification**: Multi-level verification (Basic → Deep)
4. **Automatic Repair**: Index rebuilding, UTXO reconstruction
5. **Backup/Restore**: Full and incremental backup support

## Data Directory Structure

```
/var/lib/supernova/
├── chaindata/           # Blockchain data (blocks, headers)
├── utxos/               # UTXO set
├── indexes/             # Block height and transaction indexes
├── wal/                 # Write-ahead log
├── checkpoints/         # Recovery checkpoints
└── backups/             # Local backup storage
```

## Backup Procedures

### Full Backup

A full backup includes all blockchain data and can be used for complete restoration.

```bash
# Method 1: Using supernova-cli (recommended)
supernova-cli backup create --mode full --destination /backup/supernova/

# Method 2: Manual backup (requires node stop)
systemctl stop supernova-node
tar -czvf supernova-backup-$(date +%Y%m%d-%H%M%S).tar.gz /var/lib/supernova/
systemctl start supernova-node

# Method 3: Live backup with snapshot (if filesystem supports it)
# For ZFS:
zfs snapshot tank/supernova@backup-$(date +%Y%m%d)
zfs send tank/supernova@backup-$(date +%Y%m%d) > /backup/supernova-snapshot.zfs
```

### Incremental Backup

Incremental backups only store changes since the last backup, saving space and time.

```bash
# Using supernova-cli
supernova-cli backup create --mode incremental --destination /backup/supernova/

# Using rsync (node can remain running)
rsync -av --delete \
  --exclude='wal/*' \
  /var/lib/supernova/ /backup/supernova/
```

### Backup Best Practices

1. **Regular Schedule**: Daily incremental, weekly full backups
2. **Multiple Copies**: Store backups in multiple locations
3. **Verify Backups**: Regularly test restoration
4. **Retention Policy**: Keep at least 7 daily, 4 weekly backups
5. **Encryption**: Encrypt backups at rest

```bash
# Example backup script with encryption
#!/bin/bash
DATE=$(date +%Y%m%d)
BACKUP_DIR="/backup/supernova"

# Create backup
supernova-cli backup create --mode full --destination "$BACKUP_DIR/$DATE"

# Encrypt backup
gpg --symmetric --cipher-algo AES256 \
  -o "$BACKUP_DIR/$DATE.tar.gz.gpg" \
  "$BACKUP_DIR/$DATE.tar.gz"

# Upload to remote storage
aws s3 cp "$BACKUP_DIR/$DATE.tar.gz.gpg" s3://my-backup-bucket/supernova/
```

## Restore Procedures

### From Full Backup

```bash
# 1. Stop the node
systemctl stop supernova-node

# 2. Remove corrupted data (CAUTION: make sure backup is valid first!)
rm -rf /var/lib/supernova/*

# 3. Restore from backup
tar -xzvf supernova-backup-YYYYMMDD-HHMMSS.tar.gz -C /

# 4. Start the node
systemctl start supernova-node

# 5. Verify restoration
supernova-cli repair check --level standard
```

### From Incremental Backup

```bash
# 1. Restore latest full backup first
tar -xzvf supernova-full-backup.tar.gz -C /

# 2. Apply incremental backups in order
for backup in /backup/supernova/incremental/*.tar.gz; do
  tar -xzvf "$backup" -C /var/lib/supernova/ --overwrite
done

# 3. Verify
supernova-cli repair check --level full
```

### Partial Restoration

Restore specific components without full restoration:

```bash
# Restore only UTXO set
tar -xzvf backup.tar.gz -C /var/lib/supernova/ --strip-components=2 "var/lib/supernova/utxos"

# Restore only indexes
tar -xzvf backup.tar.gz -C /var/lib/supernova/ --strip-components=2 "var/lib/supernova/indexes"
```

## Integrity Checking

### Verification Levels

| Level | What's Checked | Duration | When to Use |
|-------|---------------|----------|-------------|
| Basic | Database structure | Seconds | Quick health check |
| Standard | + Block chain continuity | Minutes | After restart |
| Full | + UTXO set consistency | 10-30 min | Weekly maintenance |
| Deep | + Cryptographic proofs | Hours | After incidents |

```bash
# Quick check (Basic)
supernova-cli repair check

# Standard check
supernova-cli repair check --level standard

# Full check
supernova-cli repair check --level full

# Deep verification (slow but thorough)
supernova-cli repair check --level deep
```

### Interpreting Results

```
✓ Database structure OK
✓ Block chain continuity OK (height: 1,234,567)
✓ UTXO set consistency OK (12,345,678 entries)
✓ Cryptographic verification OK

Overall: PASSED
Duration: 15m 23s
```

If issues are found:

```
✗ UTXO set inconsistency detected
  - 23 orphaned entries found
  - 5 missing entries detected
  
Recommendation: Run 'supernova-cli repair rebuild-utxo'
```

## Recovery from Corruption

### Automatic Recovery

Supernova automatically attempts recovery on startup if corruption is detected:

1. Replay WAL entries
2. Verify recovered state
3. Rebuild indexes if needed
4. Continue normal operation

### Manual Recovery

If automatic recovery fails:

```bash
# Step 1: Check integrity to identify issues
supernova-cli repair check --level full

# Step 2: Attempt automatic repair
supernova-cli repair auto

# Step 3: If blocks are corrupted, try to repair from peers
supernova-cli repair repair-blocks

# Step 4: If UTXO set is corrupted, rebuild from blocks
supernova-cli repair rebuild-utxo

# Step 5: If indexes are corrupted, rebuild them
supernova-cli repair rebuild-indexes

# Step 6: If all else fails, restore from backup
supernova-cli backup restore --source /backup/supernova/latest
```

### Recovery Commands Reference

| Command | Description | Duration |
|---------|-------------|----------|
| `repair check` | Check integrity | Variable |
| `repair auto` | Attempt automatic repair | Minutes |
| `repair repair-blocks` | Re-fetch corrupted blocks from peers | Variable |
| `repair rebuild-utxo` | Rebuild UTXO set from blocks | Hours |
| `repair rebuild-indexes` | Rebuild block/tx indexes | Minutes |
| `repair revert-checkpoint` | Revert to last valid checkpoint | Minutes |

### UTXO Set Rebuild

The UTXO set can be completely rebuilt from the block chain:

```bash
# Full rebuild from genesis
supernova-cli repair rebuild-utxo --from-height 0

# Rebuild from a specific height
supernova-cli repair rebuild-utxo --from-height 1000000

# Rebuild with progress reporting
supernova-cli repair rebuild-utxo --progress
```

**Note**: UTXO rebuild is resource-intensive. Expect:
- Duration: 1-4 hours depending on chain height
- Disk I/O: High sustained read/write
- Memory: ~2-4 GB peak usage

## Preventive Maintenance

### Regular Tasks

```bash
# Weekly: Run standard integrity check
0 3 * * 0 supernova-cli repair check --level standard >> /var/log/supernova/integrity.log

# Monthly: Run full integrity check
0 4 1 * * supernova-cli repair check --level full >> /var/log/supernova/integrity.log

# Daily: Create incremental backup
0 2 * * * /opt/supernova/backup-incremental.sh

# Weekly: Create full backup
0 2 * * 0 /opt/supernova/backup-full.sh
```

### Database Compaction

Periodic compaction improves performance and reclaims space:

```bash
# Compact database (node can remain running, but performance may degrade)
supernova-cli repair compact

# Or trigger during low-activity hours
0 4 * * 3 supernova-cli repair compact >> /var/log/supernova/compact.log
```

## Troubleshooting

### Common Issues

#### 1. Node won't start after crash

```bash
# Check for WAL corruption
supernova-cli repair check-wal

# If WAL is corrupted, recover from last checkpoint
supernova-cli repair revert-checkpoint
```

#### 2. "Database corrupted" error

```bash
# Try automatic repair first
supernova-cli repair auto

# If that fails, identify the issue
supernova-cli repair check --level full --verbose

# Repair based on output
```

#### 3. "UTXO mismatch" error

```bash
# This indicates UTXO set doesn't match blockchain
# Rebuild from the problematic height
supernova-cli repair rebuild-utxo --from-height <height>
```

#### 4. Sync stalled

```bash
# Check for peer connectivity
supernova-cli getpeerinfo

# Verify local chain integrity
supernova-cli repair check

# If corrupted, repair and resync
```

### Emergency Contacts

If experiencing data loss or security incidents:

1. **Preserve evidence**: Don't delete or modify data
2. **Document**: Note what happened and when
3. **Report**: Follow incident response procedures
4. See: [Incident Response Guide](INCIDENT_RESPONSE.md)

## Recovery Checkpoints

Supernova creates recovery checkpoints at regular intervals:

- Every 10,000 blocks (configurable)
- On clean shutdown
- Before major operations

List available checkpoints:

```bash
supernova-cli repair list-checkpoints
```

Revert to a checkpoint:

```bash
supernova-cli repair revert-checkpoint --height 1230000
```

## Metrics and Monitoring

### Health Check Endpoints

```bash
# Liveness probe (always returns 200 if process is running)
curl http://localhost:8332/health/live

# Readiness probe (returns 200 only when fully operational)
curl http://localhost:8332/health/ready
```

### Key Metrics to Monitor

| Metric | Warning Threshold | Critical Threshold |
|--------|------------------|-------------------|
| `supernova_storage_write_errors` | > 0 in 5m | > 10 in 5m |
| `supernova_db_corruption_detected` | Any occurrence | Any occurrence |
| `supernova_backup_age_seconds` | > 86400 (1 day) | > 604800 (1 week) |
| `supernova_utxo_set_size` | N/A | Sudden decrease |

---

*Last Updated: 2024*
*Document Version: 1.0*


# Supernova Operational Runbooks

This directory contains operational runbooks for the Supernova blockchain network. Each runbook provides step-by-step procedures for handling specific operational scenarios.

## Quick Reference

| Runbook | Alert Trigger | Severity | RTO |
|---------|---------------|----------|-----|
| [Consensus Halt](./CONSENSUS_HALT.md) | `ConsensusHalt` | P0 | 1 hour |
| [Network Partition](./NETWORK_PARTITION.md) | `ChainTipDivergence` | P0 | 2 hours |
| [Database Corruption](./DATABASE_CORRUPTION.md) | `UTXOSetCorruption` | P0 | 4 hours |
| [Quantum Emergency](./QUANTUM_EMERGENCY.md) | `QuantumCanaryTriggered` | P0 | 1 hour |
| [Fork Resolution](./FORK_RESOLUTION.md) | `ForkDetected` | P0-P1 | 2 hours |

## Runbook Structure

Each runbook follows a standard format:

1. **Alert** - Which alert triggers this runbook
2. **Severity** - Priority level (P0-P3)
3. **Description** - What the issue is
4. **RTO/RPO** - Recovery objectives
5. **Symptoms** - How to identify the issue
6. **Immediate Actions** - First steps (5-15 minutes)
7. **Resolution Procedures** - Detailed fix steps
8. **Recovery Verification** - How to confirm resolution
9. **Prevention** - How to prevent recurrence
10. **Escalation Path** - When and how to escalate
11. **Related Documentation** - Links to other resources

## Usage Guidelines

### Before an Incident

1. **Familiarize yourself** with runbooks during on-call handoff
2. **Verify access** to all systems mentioned in runbooks
3. **Test commands** in staging environment if unfamiliar
4. **Know escalation contacts** before you need them

### During an Incident

1. **Identify the runbook** based on alert name
2. **Follow steps in order** - don't skip
3. **Document actions** in incident ticket
4. **Communicate progress** via designated channels
5. **Escalate early** if uncertain

### After an Incident

1. **Complete verification steps** before closing
2. **Document any deviations** from runbook
3. **Propose updates** if runbook was unclear or incomplete
4. **Schedule post-mortem** for significant incidents

## Common Commands Reference

### Node Status

```bash
# Overall blockchain status
supernova-cli getblockchaininfo

# Peer connections
supernova-cli getpeerinfo | jq 'length'

# Memory pool status
supernova-cli getmempoolinfo

# Mining status
supernova-cli getmininginfo
```

### Health Checks

```bash
# Quick integrity check
supernova-cli repair check

# Full integrity check
supernova-cli repair check --level full

# Chain tips (fork detection)
supernova-cli getchaintips
```

### Logs

```bash
# Recent logs
journalctl -u supernova-node --since "30 minutes ago"

# Error logs only
journalctl -u supernova-node --since "1 hour ago" | grep -i error

# Follow live logs
journalctl -u supernova-node -f
```

## Alert Severity Mapping

| Prometheus Alert | Severity | Primary Runbook |
|------------------|----------|-----------------|
| `ConsensusHalt` | P0 | [Consensus Halt](./CONSENSUS_HALT.md) |
| `ForkDetected` | P0 | [Fork Resolution](./FORK_RESOLUTION.md) |
| `ChainTipDivergence` | P0 | [Network Partition](./NETWORK_PARTITION.md) |
| `UTXOSetCorruption` | P0 | [Database Corruption](./DATABASE_CORRUPTION.md) |
| `QuantumCanaryTriggered` | P0 | [Quantum Emergency](./QUANTUM_EMERGENCY.md) |
| `BackupCriticallyOld` | P1 | [Backup Restore](../BACKUP_RESTORE.md) |
| `DatabaseCapacityCritical` | P1 | [Database Corruption](./DATABASE_CORRUPTION.md) |

## Contributing

To update a runbook:

1. Create a branch from `main`
2. Update the runbook file
3. Test commands in staging
4. Submit PR with description of changes
5. Get review from operations team

## Related Documentation

- [Incident Response Plan](../INCIDENT_RESPONSE.md)
- [Disaster Recovery Plan](../DISASTER_RECOVERY.md)
- [Backup and Restore](../BACKUP_RESTORE.md)
- [Prometheus Alert Rules](../../../deployment/monitoring/prometheus/alerts.yml)

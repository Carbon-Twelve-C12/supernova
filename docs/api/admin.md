# Admin API Reference

This document describes the Admin API endpoints provided by the SuperNova blockchain node, which allow for node administration and management.

## Endpoints

All admin API endpoints are prefixed with `/api/v1/admin`.

## Authentication

Admin endpoints require authentication with admin privileges. Include an API key with admin access in the `Authorization` header:

```
Authorization: Bearer ADMIN_API_KEY
```

## Response Format

All API responses follow a standard format:

```json
{
  "success": true,
  "data": {...}
}
```

In case of an error:

```json
{
  "success": false,
  "error": "Error message"
}
```

## Admin Endpoints

### GET /api/v1/admin/status

Get detailed node status information.

**Parameters:** None

**Response:**
```json
{
  "success": true,
  "data": {
    "status": "running",
    "uptime": 1209600,
    "version": "1.0.0",
    "network": "mainnet",
    "peers_connected": 42,
    "blocks_processed": 123456,
    "memory_usage": {
      "used_mb": 1024,
      "total_mb": 8192,
      "percent": 12.5
    },
    "disk_usage": {
      "used_gb": 45.2,
      "total_gb": 500.0,
      "percent": 9.04
    },
    "cpu_usage": {
      "percent": 15.3,
      "threads": 8
    },
    "last_block": {
      "height": 123456,
      "hash": "a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0c1d2e3f4a5b6c7d8e9f0a1b2",
      "time": "2025-04-15T12:34:56Z"
    },
    "sync_status": {
      "synced": true,
      "progress": 100.0,
      "current_height": 123456,
      "highest_block": 123456,
      "sync_speed_blocks_per_second": 0
    },
    "warnings": []
  }
}
```

### GET /api/v1/admin/config

Get the node configuration.

**Parameters:** None

**Response:**
```json
{
  "success": true,
  "data": {
    "node": {
      "chain_id": "supernova-mainnet",
      "environment": "Production",
      "metrics_enabled": true,
      "metrics_port": 9000
    },
    "network": {
      "listen_addr": "/ip4/0.0.0.0/tcp/8000",
      "max_peers": 50,
      "connection_timeout": 30,
      "enable_upnp": true,
      "enable_peer_exchange": true,
      "enable_nat_traversal": true,
      "max_inbound_connections": 128,
      "max_outbound_connections": 24
    },
    "storage": {
      "db_path": "./data",
      "enable_compression": true,
      "cache_size_mb": 512,
      "backup_interval_hours": 24,
      "enable_pruning": true,
      "pruning_keep_recent": 10000
    },
    "mempool": {
      "max_size_mb": 300,
      "min_fee_rate": 1.0,
      "max_tx_per_block": 5000,
      "replace_by_fee": true,
      "max_orphan_tx": 100
    },
    "security": {
      "min_diversity_score": 0.7,
      "connection_strategy": "GeographicDiversity",
      "rate_limit_window_secs": 60,
      "rotation_interval_hours": 6,
      "min_outbound_connections": 8,
      "signature_threshold": 3,
      "enable_peer_challenges": true,
      "challenge_difficulty": 16,
      "max_connection_attempts_per_min": 5,
      "max_peers_per_subnet": 3,
      "max_inbound_ratio": 3.0
    },
    "environmental": {
      "enable_emissions_tracking": true,
      "enable_treasury": true,
      "enable_green_miner_incentives": true,
      "fee_allocation_percentage": 2.0,
      "rec_incentive_multiplier": 2.0,
      "offset_incentive_multiplier": 1.2
    },
    "monitoring": {
      "metrics_enabled": true,
      "metrics_endpoint": "0.0.0.0:9091",
      "enable_system_metrics": true,
      "enable_tracing": true,
      "trace_sampling_rate": 0.1,
      "system_metrics_interval_secs": 15
    },
    "lightning": {
      "enable": true,
      "max_channels": 100,
      "default_channel_capacity": 1000000,
      "min_htlc_value_msat": 1000,
      "max_htlc_value_msat": 100000000,
      "use_quantum_signatures": true,
      "watchtower_enabled": true
    },
    "api": {
      "bind_address": "127.0.0.1",
      "port": 8080,
      "enable_docs": true,
      "cors_allowed_origins": ["*"],
      "rate_limit": 100,
      "enable_auth": true,
      "detailed_logging": true,
      "max_json_payload_size": 5,
      "request_timeout": 30,
      "enable_jsonrpc": true,
      "jsonrpc_path": "/rpc"
    }
  }
}
```

### PUT /api/v1/admin/config

Update the node configuration.

**Request Body:**
```json
{
  "network": {
    "max_peers": 75
  },
  "security": {
    "min_diversity_score": 0.8
  }
}
```

**Response:**
```json
{
  "success": true,
  "data": {
    "updated": ["network.max_peers", "security.min_diversity_score"],
    "restart_required": false
  }
}
```

### POST /api/v1/admin/restart

Restart the node.

**Request Body (optional):**
```json
{
  "reason": "Configuration update",
  "delay_seconds": 30
}
```

**Response:**
```json
{
  "success": true,
  "data": {
    "restart_scheduled": true,
    "restart_time": "2025-04-15T12:35:26Z",
    "reason": "Configuration update"
  }
}
```

### POST /api/v1/admin/shutdown

Shutdown the node.

**Request Body (optional):**
```json
{
  "reason": "Maintenance",
  "delay_seconds": 60
}
```

**Response:**
```json
{
  "success": true,
  "data": {
    "shutdown_scheduled": true,
    "shutdown_time": "2025-04-15T12:35:56Z",
    "reason": "Maintenance"
  }
}
```

### GET /api/v1/admin/backup/list

List available backups.

**Parameters:** None

**Response:**
```json
{
  "success": true,
  "data": {
    "backups": [
      {
        "id": "backup-20250415-123456",
        "path": "/path/to/backups/supernova_backup_1681553696.db",
        "size_bytes": 1073741824,
        "created_at": "2025-04-15T12:34:56Z",
        "blockchain_height": 123456,
        "checksum": "a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6"
      },
      {
        "id": "backup-20250414-123456",
        "path": "/path/to/backups/supernova_backup_1681467296.db",
        "size_bytes": 1068237824,
        "created_at": "2025-04-14T12:34:56Z",
        "blockchain_height": 123356,
        "checksum": "b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7"
      }
    ],
    "total_count": 2,
    "total_size_bytes": 2141979648,
    "backup_directory": "/path/to/backups"
  }
}
```

### POST /api/v1/admin/backup/create

Create a new backup.

**Request Body (optional):**
```json
{
  "description": "Pre-upgrade backup"
}
```

**Response:**
```json
{
  "success": true,
  "data": {
    "backup_id": "backup-20250415-123456",
    "path": "/path/to/backups/supernova_backup_1681553696.db",
    "size_bytes": 1073741824,
    "created_at": "2025-04-15T12:34:56Z",
    "blockchain_height": 123456,
    "description": "Pre-upgrade backup",
    "checksum": "a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6"
  }
}
```

### POST /api/v1/admin/backup/restore

Restore from a backup.

**Request Body:**
```json
{
  "backup_id": "backup-20250414-123456",
  "force": false,
  "verify_before_restore": true
}
```

**Response:**
```json
{
  "success": true,
  "data": {
    "restore_initiated": true,
    "restore_id": "restore-20250415-123456",
    "backup_id": "backup-20250414-123456",
    "estimated_completion_time": "2025-04-15T12:54:56Z",
    "node_will_restart": true
  }
}
```

### GET /api/v1/admin/backup/restore/status

Get status of an ongoing restore operation.

**Parameters:**
- `restore_id` (optional): The ID of the restore operation

**Response:**
```json
{
  "success": true,
  "data": {
    "restore_id": "restore-20250415-123456",
    "backup_id": "backup-20250414-123456",
    "status": "in_progress",
    "progress_percent": 45.6,
    "started_at": "2025-04-15T12:35:00Z",
    "estimated_completion_time": "2025-04-15T12:54:56Z",
    "current_stage": "copying_files",
    "error": null
  }
}
```

### GET /api/v1/admin/logs

Get node logs.

**Parameters:**
- `level` (optional): Log level (debug, info, warn, error) (default: `info`)
- `limit` (optional): Number of logs to return (default: `100`)
- `offset` (optional): Pagination offset (default: `0`)
- `since` (optional): ISO timestamp to get logs since
- `until` (optional): ISO timestamp to get logs until
- `component` (optional): Filter by component name (e.g., `network`, `blockchain`, `mempool`)

**Response:**
```json
{
  "success": true,
  "data": {
    "logs": [
      {
        "timestamp": "2025-04-15T12:34:56Z",
        "level": "info",
        "component": "blockchain",
        "message": "Block #123456 added to chain: a1b2c3...d4e5f6",
        "details": {}
      },
      {
        "timestamp": "2025-04-15T12:34:55Z",
        "level": "info",
        "component": "mempool",
        "message": "Added transaction to mempool: 7f8g9h...i0j1k2",
        "details": {
          "size": 500,
          "fee": 0.00012,
          "feerate": 2.4
        }
      }
    ],
    "total_count": 2354,
    "filter_applied": {
      "level": "info",
      "limit": 100,
      "offset": 0,
      "component": null,
      "since": null,
      "until": null
    }
  }
}
```

### GET /api/v1/admin/logs/download

Download logs as a file.

**Parameters:**
- `level` (optional): Minimum log level (debug, info, warn, error) (default: `info`)
- `days` (optional): Number of days to include (default: `1`)
- `format` (optional): Output format (txt, json, csv) (default: `txt`)
- `component` (optional): Filter by component name

**Response:**
Binary file download

### GET /api/v1/admin/metrics

Get detailed node metrics.

**Parameters:**
- `components` (optional): Comma-separated list of components (system, blockchain, network, mempool, environmental)

**Response:**
```json
{
  "success": true,
  "data": {
    "timestamp": "2025-04-15T12:34:56Z",
    "system": {
      "cpu_usage_percent": 15.3,
      "memory_usage_mb": 1024,
      "disk_usage_gb": 45.2,
      "disk_io_reads_per_sec": 125,
      "disk_io_writes_per_sec": 75,
      "network_inbound_mbps": 1.5,
      "network_outbound_mbps": 2.3,
      "open_file_descriptors": 256,
      "uptime_seconds": 1209600
    },
    "blockchain": {
      "height": 123456,
      "transactions_processed": 2500000,
      "avg_block_time_seconds": 600,
      "current_difficulty": 12345678901234,
      "estimated_hashrate": 12000000000000,
      "chain_size_gb": 32.5,
      "utxo_set_size": 1250000,
      "fork_count": 3
    },
    "network": {
      "peers_connected": 42,
      "inbound_peers": 15,
      "outbound_peers": 27,
      "bandwidth_usage_inbound_mb": 1250,
      "bandwidth_usage_outbound_mb": 2340,
      "messages_received": 15000000,
      "messages_sent": 12000000,
      "average_latency_ms": 120,
      "peer_countries": {
        "US": 10,
        "DE": 8,
        "JP": 5,
        "CA": 4,
        "UK": 4,
        "OTHER": 11
      }
    },
    "mempool": {
      "transaction_count": 2500,
      "mempool_size_mb": 42.5,
      "fee_rates": {
        "min": 1.0,
        "max": 25.0,
        "median": 3.2,
        "average": 4.5
      },
      "transactions_per_second": 2.3,
      "bytes_per_second": 5600
    },
    "environmental": {
      "emissions_last_24h_kgCO2": 12.5,
      "energy_consumption_last_24h_kWh": 35.6,
      "renewable_percentage": 65.3,
      "emissions_saved_by_lightning_kgCO2": 5.2,
      "carbon_offset_balance_kgCO2": 250,
      "rec_balance_kWh": 500
    }
  }
}
```

### GET /api/v1/admin/users

List API users.

**Parameters:**
- `limit` (optional): Number of users to return (default: `100`)
- `offset` (optional): Pagination offset (default: `0`)

**Response:**
```json
{
  "success": true,
  "data": {
    "users": [
      {
        "id": "user-1",
        "username": "admin",
        "role": "admin",
        "api_key_prefix": "sk_admin_xxxxxxxx",
        "created_at": "2025-01-01T00:00:00Z",
        "last_active": "2025-04-15T10:23:45Z",
        "permissions": ["admin", "read", "write"],
        "rate_limit": 1000
      },
      {
        "id": "user-2",
        "username": "readonly",
        "role": "reader",
        "api_key_prefix": "sk_reader_xxxxxxxx",
        "created_at": "2025-02-15T00:00:00Z",
        "last_active": "2025-04-15T11:15:30Z",
        "permissions": ["read"],
        "rate_limit": 500
      }
    ],
    "total_count": 2
  }
}
```

### POST /api/v1/admin/users

Create a new API user.

**Request Body:**
```json
{
  "username": "developer",
  "role": "developer",
  "permissions": ["read", "write"],
  "rate_limit": 500
}
```

**Response:**
```json
{
  "success": true,
  "data": {
    "id": "user-3",
    "username": "developer",
    "role": "developer",
    "api_key": "sk_developer_abcdef123456789012345678901234567890",
    "api_key_prefix": "sk_developer_abcdef",
    "created_at": "2025-04-15T12:34:56Z",
    "permissions": ["read", "write"],
    "rate_limit": 500
  }
}
```

### GET /api/v1/admin/users/{user_id}

Get information about a specific API user.

**Parameters:**
- `user_id` (path): The user ID

**Response:**
```json
{
  "success": true,
  "data": {
    "id": "user-1",
    "username": "admin",
    "role": "admin",
    "api_key_prefix": "sk_admin_xxxxxxxx",
    "created_at": "2025-01-01T00:00:00Z",
    "last_active": "2025-04-15T10:23:45Z",
    "permissions": ["admin", "read", "write"],
    "rate_limit": 1000,
    "access_log": [
      {
        "timestamp": "2025-04-15T10:23:45Z",
        "endpoint": "/api/v1/admin/metrics",
        "method": "GET",
        "ip": "192.168.1.1",
        "response_code": 200
      },
      {
        "timestamp": "2025-04-15T10:22:30Z",
        "endpoint": "/api/v1/admin/status",
        "method": "GET",
        "ip": "192.168.1.1",
        "response_code": 200
      }
    ]
  }
}
```

### PUT /api/v1/admin/users/{user_id}

Update an API user.

**Parameters:**
- `user_id` (path): The user ID

**Request Body:**
```json
{
  "role": "advanced_developer",
  "permissions": ["read", "write", "debug"],
  "rate_limit": 750
}
```

**Response:**
```json
{
  "success": true,
  "data": {
    "id": "user-3",
    "username": "developer",
    "role": "advanced_developer",
    "api_key_prefix": "sk_developer_abcdef",
    "updated_at": "2025-04-15T12:45:00Z",
    "permissions": ["read", "write", "debug"],
    "rate_limit": 750
  }
}
```

### DELETE /api/v1/admin/users/{user_id}

Delete an API user.

**Parameters:**
- `user_id` (path): The user ID

**Response:**
```json
{
  "success": true,
  "data": {
    "id": "user-3",
    "username": "developer",
    "deleted": true
  }
}
```

### POST /api/v1/admin/users/{user_id}/reset-key

Reset the API key for a user.

**Parameters:**
- `user_id` (path): The user ID

**Response:**
```json
{
  "success": true,
  "data": {
    "id": "user-3",
    "username": "developer",
    "new_api_key": "sk_developer_defabc123456789012345678901234567890",
    "new_api_key_prefix": "sk_developer_defabc",
    "updated_at": "2025-04-15T12:50:00Z"
  }
}
```

### GET /api/v1/admin/security

Get security-related information and statistics.

**Parameters:** None

**Response:**
```json
{
  "success": true,
  "data": {
    "current_settings": {
      "min_diversity_score": 0.7,
      "connection_strategy": "GeographicDiversity",
      "rate_limit_window_secs": 60,
      "challenge_difficulty": 16
    },
    "diversity_score": {
      "current": 0.85,
      "target": 0.7,
      "history": [
        { "timestamp": "2025-04-15T12:00:00Z", "score": 0.84 },
        { "timestamp": "2025-04-15T11:00:00Z", "score": 0.82 },
        { "timestamp": "2025-04-15T10:00:00Z", "score": 0.8 }
      ]
    },
    "peer_distribution": {
      "geographic": {
        "North America": 35,
        "Europe": 30,
        "Asia": 25,
        "Oceania": 5,
        "South America": 3,
        "Africa": 2
      },
      "asn": {
        "AS1299": 5,
        "AS3356": 4,
        "AS174": 3,
        "OTHER": 30
      }
    },
    "banned_peers": {
      "total": 15,
      "by_reason": {
        "rate_limit_exceeded": 8,
        "invalid_messages": 4,
        "manual_ban": 2,
        "protocol_violation": 1
      },
      "recent": [
        {
          "ip": "198.51.100.1",
          "banned_at": "2025-04-15T12:30:00Z",
          "ban_expires": "2025-04-16T12:30:00Z",
          "reason": "rate_limit_exceeded"
        }
      ]
    },
    "challenge_statistics": {
      "challenges_issued": 500,
      "successful_verifications": 450,
      "failed_verifications": 50,
      "average_verification_time_ms": 250
    },
    "security_incidents": {
      "total": 3,
      "by_type": {
        "multiple_connections": 2,
        "spam_transactions": 1
      },
      "recent": [
        {
          "type": "multiple_connections",
          "timestamp": "2025-04-15T11:45:00Z",
          "description": "Multiple connection attempts from subnet 198.51.100.0/24",
          "resolution": "Temporary subnet ban applied"
        }
      ]
    }
  }
}
```

### GET /api/v1/admin/debug/pprof

Get runtime profiling data in the format expected by the pprof visualization tool.

**Parameters:**
- `profile` (optional): The profile to generate (cpu, heap, goroutine, threadcreate, block, mutex)
- `seconds` (optional): CPU profile duration in seconds (default: 30)

**Response:**
Binary file download (pprof format)

## Error Codes

- `400` - Bad Request: Invalid parameters or request
- `401` - Unauthorized: Authentication required
- `403` - Forbidden: Insufficient permissions
- `404` - Not Found: Resource not found
- `500` - Internal Server Error: Server error

## Examples

### Curl Examples

#### Get Node Status
```bash
curl -X GET "https://node.supernova.io/api/v1/admin/status" \
  -H "Authorization: Bearer ADMIN_API_KEY"
```

#### Update Configuration
```bash
curl -X PUT "https://node.supernova.io/api/v1/admin/config" \
  -H "Authorization: Bearer ADMIN_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{
    "network": {
      "max_peers": 75
    },
    "security": {
      "min_diversity_score": 0.8
    }
  }'
```

#### Create Backup
```bash
curl -X POST "https://node.supernova.io/api/v1/admin/backup/create" \
  -H "Authorization: Bearer ADMIN_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{
    "description": "Pre-upgrade backup"
  }'
``` 
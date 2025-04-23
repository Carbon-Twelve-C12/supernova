# Statistics API Reference

This document describes the Statistics API endpoints provided by the SuperNova blockchain node, which provide detailed analytics and metrics about the blockchain, network, and environmental impact.

## Endpoints

All statistics API endpoints are prefixed with `/api/v1/stats`.

## Authentication

Most statistics endpoints are publicly accessible, though some may require authentication for detailed data. To authenticate, include an API key in the `Authorization` header:

```
Authorization: Bearer YOUR_API_KEY
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

## Statistics Endpoints

### GET /api/v1/stats/blockchain

Get blockchain statistics.

**Parameters:**
- `period` (optional): Time period for statistics (day, week, month, year, all) (default: `day`)

**Response:**
```json
{
  "success": true,
  "data": {
    "height": 123456,
    "total_transactions": 25000000,
    "total_outputs": 60000000,
    "total_fees": 12500.25,
    "difficulty": 12345678901234,
    "hashrate": 12000000000000,
    "block_time_avg": 600.25,
    "block_size_avg": 750000,
    "transactions_per_block_avg": 2500,
    "utxo_set_size": 1250000,
    "blocks_last_24h": 144,
    "transactions_last_24h": 360000,
    "fees_last_24h": 125.75,
    "chain_size": {
      "blockchain_size_mb": 32768,
      "utxo_db_size_mb": 4096,
      "index_size_mb": 8192,
      "total_size_mb": 45056
    },
    "block_interval_distribution": {
      "0-300s": 15,
      "300-600s": 75,
      "600-900s": 40,
      "900-1200s": 10,
      "1200+s": 4
    },
    "transaction_volume_last_24h": 125000.5,
    "charts": {
      "block_time": [
        {"timestamp": "2025-04-15T00:00:00Z", "value": 605.3},
        {"timestamp": "2025-04-14T00:00:00Z", "value": 598.7}
      ],
      "difficulty": [
        {"timestamp": "2025-04-15T00:00:00Z", "value": 12345678901234},
        {"timestamp": "2025-04-14T00:00:00Z", "value": 12245678901234}
      ],
      "transaction_count": [
        {"timestamp": "2025-04-15T00:00:00Z", "value": 360000},
        {"timestamp": "2025-04-14T00:00:00Z", "value": 355000}
      ]
    }
  }
}
```

### GET /api/v1/stats/network

Get network statistics.

**Parameters:**
- `period` (optional): Time period for statistics (day, week, month, year, all) (default: `day`)

**Response:**
```json
{
  "success": true,
  "data": {
    "connected_peers": 42,
    "peer_versions": {
      "1.0.0": 30,
      "0.9.0": 10,
      "other": 2
    },
    "peer_countries": {
      "US": 10,
      "DE": 8,
      "JP": 5,
      "CA": 4,
      "UK": 4,
      "OTHER": 11
    },
    "connection_types": {
      "ipv4": 35,
      "ipv6": 5,
      "tor": 2
    },
    "bandwidth_usage": {
      "total_received_gb": 1250.5,
      "total_sent_gb": 2340.8,
      "received_last_24h_gb": 15.2,
      "sent_last_24h_gb": 25.7,
      "current_inbound_rate_kbps": 1500,
      "current_outbound_rate_kbps": 2300
    },
    "latency": {
      "average_ms": 120,
      "median_ms": 95,
      "p95_ms": 250,
      "min_ms": 15,
      "max_ms": 450
    },
    "message_stats": {
      "total_received": 15000000,
      "total_sent": 12000000,
      "by_type": {
        "addr": 150000,
        "block": 450000,
        "tx": 8000000,
        "ping": 3500000,
        "pong": 3500000,
        "other": 400000
      }
    },
    "propagation_times": {
      "block_propagation_time_avg_ms": 850,
      "tx_propagation_time_avg_ms": 350
    },
    "charts": {
      "peer_count": [
        {"timestamp": "2025-04-15T00:00:00Z", "value": 42},
        {"timestamp": "2025-04-14T00:00:00Z", "value": 40}
      ],
      "bandwidth_usage": [
        {"timestamp": "2025-04-15T00:00:00Z", "inbound_gb": 15.2, "outbound_gb": 25.7},
        {"timestamp": "2025-04-14T00:00:00Z", "inbound_gb": 14.8, "outbound_gb": 24.9}
      ]
    }
  }
}
```

### GET /api/v1/stats/mempool

Get mempool statistics.

**Parameters:**
- `detailed` (optional): Whether to include detailed fee distribution (default: `false`)

**Response:**
```json
{
  "success": true,
  "data": {
    "transaction_count": 2500,
    "size_bytes": 44568000,
    "total_fees": 0.25,
    "fee_rates": {
      "min": 1.0,
      "max": 25.0,
      "median": 3.2,
      "average": 4.5,
      "p10": 1.5,
      "p25": 2.1,
      "p75": 6.8,
      "p90": 12.3
    },
    "fee_distribution": {
      "1-2 sat/byte": 500,
      "2-5 sat/byte": 1200,
      "5-10 sat/byte": 600,
      "10-20 sat/byte": 150,
      "20+ sat/byte": 50
    },
    "transaction_size_distribution": {
      "0-250 bytes": 450,
      "250-500 bytes": 1300,
      "500-1000 bytes": 650,
      "1000+ bytes": 100
    },
    "age_distribution": {
      "0-10 min": 750,
      "10-30 min": 950,
      "30-60 min": 450,
      "60+ min": 350
    },
    "transactions_per_second": 2.3,
    "bytes_per_second": 5600,
    "mempool_history": [
      {"timestamp": "2025-04-15T12:00:00Z", "tx_count": 2500, "size_mb": 42.5},
      {"timestamp": "2025-04-15T11:00:00Z", "tx_count": 2300, "size_mb": 40.1},
      {"timestamp": "2025-04-15T10:00:00Z", "tx_count": 2200, "size_mb": 38.5}
    ]
  }
}
```

### GET /api/v1/stats/mining

Get mining statistics.

**Parameters:**
- `period` (optional): Time period for statistics (day, week, month, year, all) (default: `day`)

**Response:**
```json
{
  "success": true,
  "data": {
    "hashrate": {
      "current": 12000000000000,
      "average_24h": 11850000000000,
      "highest_ever": 15000000000000,
      "network_hashrate_distribution": {
        "pool_1": 25.5,
        "pool_2": 18.2,
        "pool_3": 15.0,
        "pool_4": 12.3,
        "pool_5": 10.8,
        "others": 18.2
      }
    },
    "difficulty": {
      "current": 12345678901234,
      "next_estimated": 12395678901234,
      "last_adjustment_percent": 3.5,
      "next_adjustment_estimate_percent": 0.4,
      "next_adjustment_block": 123480,
      "next_adjustment_time_estimate": "2025-04-16T15:30:00Z"
    },
    "block_stats": {
      "last_block_time": "2025-04-15T12:34:56Z",
      "last_block_height": 123456,
      "last_block_hash": "a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0c1d2e3f4a5b6c7d8e9f0a1b2",
      "time_since_last_block_seconds": 35,
      "blocks_last_24h": 144,
      "empty_blocks_last_24h": 2,
      "orphaned_blocks_last_24h": 1
    },
    "rewards": {
      "last_block_reward": 6.25,
      "fees_last_24h": 125.75,
      "total_rewards_last_24h": 1025.75,
      "average_fees_per_block": 0.873
    },
    "charts": {
      "hashrate": [
        {"timestamp": "2025-04-15T00:00:00Z", "value": 12000000000000},
        {"timestamp": "2025-04-14T00:00:00Z", "value": 11800000000000}
      ],
      "difficulty": [
        {"timestamp": "2025-04-15T00:00:00Z", "value": 12345678901234},
        {"timestamp": "2025-04-14T00:00:00Z", "value": 12245678901234}
      ],
      "block_interval": [
        {"timestamp": "2025-04-15T00:00:00Z", "value": 602},
        {"timestamp": "2025-04-14T00:00:00Z", "value": 610}
      ]
    }
  }
}
```

### GET /api/v1/stats/environmental

Get environmental impact statistics.

**Parameters:**
- `period` (optional): Time period for statistics (day, week, month, year, all) (default: `day`)

**Response:**
```json
{
  "success": true,
  "data": {
    "energy_consumption": {
      "current_kw": 1450000,
      "daily_kwh": 34800000,
      "annual_estimate_twh": 12.7,
      "energy_per_transaction_kwh": 96.7
    },
    "carbon_emissions": {
      "daily_tons_co2": 12500,
      "annual_estimate_tons_co2": 4562500,
      "carbon_per_transaction_kg_co2": 34.7
    },
    "renewable_energy": {
      "percentage": 65.3,
      "renewable_by_source": {
        "solar": 25.5,
        "hydro": 18.2,
        "wind": 15.0,
        "geothermal": 4.3,
        "other": 2.3
      }
    },
    "environmental_treasury": {
      "current_balance": 125000,
      "total_collected": 750000,
      "total_spent": 625000,
      "rec_purchased_mwh": 500000,
      "carbon_offsets_purchased_tons": 250000
    },
    "green_mining_metrics": {
      "verified_green_miners_count": 125,
      "green_hashrate_percentage": 68.5,
      "total_green_mining_rewards": 850000,
      "total_discount_value": 17000
    },
    "lightning_network_savings": {
      "transactions_via_lightning_24h": 1250000,
      "energy_saved_kwh_24h": 120875000,
      "carbon_saved_tons_24h": 43500
    },
    "charts": {
      "energy_consumption": [
        {"timestamp": "2025-04-15T00:00:00Z", "value": 34800000},
        {"timestamp": "2025-04-14T00:00:00Z", "value": 34200000}
      ],
      "carbon_emissions": [
        {"timestamp": "2025-04-15T00:00:00Z", "value": 12500},
        {"timestamp": "2025-04-14T00:00:00Z", "value": 12300}
      ],
      "renewable_percentage": [
        {"timestamp": "2025-04-15T00:00:00Z", "value": 65.3},
        {"timestamp": "2025-04-14T00:00:00Z", "value": 64.8}
      ]
    },
    "comparisons": {
      "grid_total_percentage": 0.054,
      "bitcoin_energy_equivalent": 0.85,
      "country_equivalent": "Finland"
    }
  }
}
```

### GET /api/v1/stats/lightning

Get Lightning Network statistics.

**Parameters:**
- `period` (optional): Time period for statistics (day, week, month, year, all) (default: `day`)

**Response:**
```json
{
  "success": true,
  "data": {
    "network_summary": {
      "total_nodes": 15000,
      "total_channels": 50000,
      "total_capacity": 5000000000,
      "total_transactions_24h": 1250000,
      "average_fee_rate": 0.000001,
      "median_fee_rate": 0.0000005,
      "average_channel_size": 10000000,
      "median_channel_size": 5000000
    },
    "node_statistics": {
      "top_nodes_by_channels": [
        {"alias": "node1", "public_key": "0373...cdf2", "channels": 2500},
        {"alias": "node2", "public_key": "0284...ae45", "channels": 2200}
      ],
      "top_nodes_by_capacity": [
        {"alias": "node3", "public_key": "03a1...f3e5", "capacity": 250000000},
        {"alias": "node4", "public_key": "02c9...b3d4", "capacity": 200000000}
      ],
      "node_count_history": [
        {"timestamp": "2025-04-15T00:00:00Z", "value": 15000},
        {"timestamp": "2025-04-14T00:00:00Z", "value": 14950}
      ]
    },
    "channel_statistics": {
      "channels_opened_24h": 500,
      "channels_closed_24h": 120,
      "capacity_added_24h": 100000000,
      "capacity_removed_24h": 25000000,
      "channel_size_distribution": {
        "0-1M": 15000,
        "1M-5M": 20000,
        "5M-10M": 10000,
        "10M-50M": 4000,
        "50M+": 1000
      },
      "channel_count_history": [
        {"timestamp": "2025-04-15T00:00:00Z", "value": 50000},
        {"timestamp": "2025-04-14T00:00:00Z", "value": 49620}
      ]
    },
    "payment_statistics": {
      "transactions_per_second_avg": 14.5,
      "total_transaction_volume_24h": 750000000,
      "average_transaction_value": 600000,
      "median_transaction_value": 150000,
      "fee_revenue_24h": 125000,
      "payment_amount_distribution": {
        "0-1000": 450000,
        "1000-10000": 350000,
        "10000-100000": 300000,
        "100000-1000000": 125000,
        "1000000+": 25000
      }
    },
    "environmental_impact": {
      "energy_per_transaction_wh": 0.075,
      "carbon_per_transaction_g": 0.027,
      "energy_saved_vs_onchain_kwh": 120875000,
      "carbon_saved_vs_onchain_tons": 43500
    },
    "charts": {
      "transactions_per_day": [
        {"timestamp": "2025-04-15T00:00:00Z", "value": 1250000},
        {"timestamp": "2025-04-14T00:00:00Z", "value": 1225000}
      ],
      "total_capacity": [
        {"timestamp": "2025-04-15T00:00:00Z", "value": 5000000000},
        {"timestamp": "2025-04-14T00:00:00Z", "value": 4925000000}
      ],
      "fee_rates": [
        {"timestamp": "2025-04-15T00:00:00Z", "value": 0.000001},
        {"timestamp": "2025-04-14T00:00:00Z", "value": 0.0000011}
      ]
    }
  }
}
```

### GET /api/v1/stats/daily-summary

Get a summary of key statistics for a specific day.

**Parameters:**
- `date` (optional): Date for summary (YYYY-MM-DD) (default: yesterday)

**Response:**
```json
{
  "success": true,
  "data": {
    "date": "2025-04-14",
    "blockchain": {
      "blocks_mined": 144,
      "transactions_processed": 355000,
      "avg_block_time_seconds": 598.7,
      "avg_block_size_bytes": 725000,
      "total_fees": 124.37,
      "difficulty": 12245678901234
    },
    "network": {
      "avg_connected_peers": 40,
      "bandwidth_usage_gb": {"inbound": 14.8, "outbound": 24.9},
      "avg_propagation_time_ms": {"blocks": 825, "transactions": 340}
    },
    "mempool": {
      "avg_tx_count": 2200,
      "avg_size_mb": 38.5,
      "avg_fee_rate": 4.3
    },
    "mining": {
      "avg_hashrate": 11800000000000,
      "orphaned_blocks": 2,
      "empty_blocks": 1
    },
    "environmental": {
      "energy_consumption_kwh": 34200000,
      "carbon_emissions_tons": 12300,
      "renewable_percentage": 64.8
    },
    "lightning": {
      "transactions": 1225000,
      "channels_opened": 450,
      "channels_closed": 150,
      "fee_revenue": 122500
    }
  }
}
```

### GET /api/v1/stats/trends

Get trend analysis for key metrics.

**Parameters:**
- `period` (optional): Time period for trends (week, month, year) (default: `month`)
- `metrics` (optional): Comma-separated list of metrics to include (default: all)

**Response:**
```json
{
  "success": true,
  "data": {
    "period": "month",
    "start_date": "2025-03-15",
    "end_date": "2025-04-15",
    "trends": {
      "transactions_per_day": {
        "start_value": 325000,
        "end_value": 360000,
        "change_percent": 10.8,
        "trend": "increasing",
        "trend_strength": "moderate"
      },
      "hashrate": {
        "start_value": 10500000000000,
        "end_value": 12000000000000,
        "change_percent": 14.3,
        "trend": "increasing",
        "trend_strength": "strong"
      },
      "fee_rates": {
        "start_value": 3.8,
        "end_value": 4.5,
        "change_percent": 18.4,
        "trend": "increasing",
        "trend_strength": "strong"
      },
      "mempool_size": {
        "start_value": 35.0,
        "end_value": 42.5,
        "change_percent": 21.4,
        "trend": "increasing",
        "trend_strength": "strong"
      },
      "energy_consumption": {
        "start_value": 33000000,
        "end_value": 34800000,
        "change_percent": 5.5,
        "trend": "increasing",
        "trend_strength": "mild"
      },
      "renewable_percentage": {
        "start_value": 62.5,
        "end_value": 65.3,
        "change_percent": 4.5,
        "trend": "increasing",
        "trend_strength": "mild"
      },
      "lightning_transactions": {
        "start_value": 950000,
        "end_value": 1250000,
        "change_percent": 31.6,
        "trend": "increasing",
        "trend_strength": "very strong"
      }
    },
    "correlations": {
      "hashrate_vs_difficulty": 0.97,
      "transaction_count_vs_fees": 0.76,
      "mempool_size_vs_fee_rate": 0.89,
      "energy_consumption_vs_hashrate": 0.94,
      "lightning_adoption_vs_onchain_fees": 0.82
    },
    "seasonality": {
      "transaction_count": {
        "daily_pattern": "Peaks on weekdays, lows on weekends",
        "weekly_pattern": "Highest on Tuesdays, lowest on Sundays"
      },
      "fee_rates": {
        "daily_pattern": "Peaks midday UTC, lows during Asian market hours",
        "weekly_pattern": "Highest on Tuesday-Wednesday, lowest on Saturday-Sunday"
      }
    }
  }
}
```

## Error Codes

- `400` - Bad Request: Invalid parameters or request
- `401` - Unauthorized: Authentication required for restricted statistics
- `403` - Forbidden: Insufficient permissions
- `404` - Not Found: Data not found for specified parameters
- `500` - Internal Server Error: Server error

## Examples

### Curl Examples

#### Get Blockchain Statistics
```bash
curl -X GET "https://node.supernova.io/api/v1/stats/blockchain?period=week"
```

#### Get Environmental Impact Statistics
```bash
curl -X GET "https://node.supernova.io/api/v1/stats/environmental"
```

#### Get Trends Analysis
```bash
curl -X GET "https://node.supernova.io/api/v1/stats/trends?period=month&metrics=transactions_per_day,hashrate,renewable_percentage"
``` 
# SuperNova Integration Guide for Exchanges and Service Providers

This guide provides detailed information for exchanges, wallets, and other service providers looking to integrate with the SuperNova blockchain. It covers the essential aspects needed for a successful integration.

## Table of Contents

- [Introduction](#introduction)
- [API Overview](#api-overview)
- [Node Setup](#node-setup)
- [Basic Integration](#basic-integration)
- [Advanced Features](#advanced-features)
- [Security Considerations](#security-considerations)
- [Environmental Features](#environmental-features)
- [Common Integration Patterns](#common-integration-patterns)
- [Testing](#testing)
- [Support](#support)

## Introduction

SuperNova is a modern blockchain implementation with advanced features including quantum-resistant cryptography, environmental impact tracking, and Lightning Network support. This guide aims to help third-party services integrate with SuperNova effectively.

### Key Features

- **UTXO-based** blockchain with Bitcoin-compatible architecture
- **Quantum-resistant signatures** (optional) for future-proof security
- **Environmental impact tracking** and incentives for green mining
- **Lightning Network** support for fast, off-chain transactions
- **Comprehensive API** with both RESTful and JSON-RPC interfaces

## API Overview

SuperNova provides two types of APIs:

1. **RESTful API** - A modern, resource-oriented API using standard HTTP methods
2. **JSON-RPC API** - A Bitcoin-compatible API following the JSON-RPC 2.0 specification

### Documentation

For complete API documentation, refer to:

- [API Reference](./api_reference.md) - Complete API reference
- [Blockchain API](./api/blockchain.md) - Block and transaction endpoints
- [Wallet API](./api/wallet.md) - Wallet management endpoints
- [Statistics API](./api/statistics.md) - Blockchain analytics endpoints

## Node Setup

### System Requirements

- **CPU**: 4+ cores
- **RAM**: 8+ GB
- **Storage**: 500+ GB SSD
- **Network**: 100+ Mbps connection
- **OS**: Linux (Ubuntu 22.04+ recommended), macOS, or Windows 10/11

### Installation

```bash
# Clone the repository
git clone https://github.com/username/supernova.git
cd supernova

# Build from source
cargo build --release

# Create a configuration file
cp config/node.example.toml config/node.toml

# Edit the configuration as needed
# nano config/node.toml

# Start the node
./target/release/node
```

### Configuration

Key settings in `node.toml` for service providers:

```toml
[node]
chain_id = "supernova-mainnet"
environment = "Production"
metrics_enabled = true

[network]
listen_addr = "/ip4/0.0.0.0/tcp/8000"
max_peers = 100
connection_timeout = 30

[api]
bind_address = "127.0.0.1"  # Change to 0.0.0.0 to allow external connections
port = 8080
enable_auth = true

[wallet]
enable = true
auto_backup = true
backup_interval_hours = 24
```

### Security Recommendations

- Run the node on a dedicated server with firewall
- Limit API access using `enable_auth` and API keys
- Use a reverse proxy (like Nginx) for TLS termination
- Set up monitoring and alerts

## Basic Integration

### Address Generation

SuperNova uses standard Bitcoin-compatible addresses with optional quantum-resistant extensions.

#### Classical Addresses

For basic integration, you can use classical addresses which start with "sn1" (for mainnet) or "snt1" (for testnet).

Example API call to generate a new address:

```bash
curl -X POST "https://node.supernova.io/api/v1/wallet/addresses/new" \
  -H "Authorization: Bearer YOUR_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{
    "label": "Customer deposit address",
    "type": "receive"
  }'
```

Response:
```json
{
  "success": true,
  "data": {
    "address": "sn1qkl4hvc5zw8ru6tpzj2lf0u4s5jx9rgwep2j5d5",
    "type": "receive",
    "hd_path": "m/84'/0'/0'/0/1",
    "label": "Customer deposit address"
  }
}
```

#### Quantum-resistant Addresses

For future-proof security, you can use quantum-resistant addresses which start with "snq" (for mainnet) or "sntq" (for testnet).

```bash
curl -X POST "https://node.supernova.io/api/v1/wallet/addresses/new" \
  -H "Authorization: Bearer YOUR_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{
    "label": "Customer deposit address",
    "type": "receive",
    "quantum_resistant": true
  }'
```

Response:
```json
{
  "success": true,
  "data": {
    "address": "snq1q9z5z9wygy0yt2l94rzp6rh89w703c8k37q3urp8r8s9w97hr7j88s5mht0",
    "type": "receive",
    "hd_path": "m/84'/0'/0'/0/1",
    "label": "Customer deposit address",
    "quantum_resistant": true
  }
}
```

### Deposit Detection

To detect incoming deposits, you can:

1. **Subscribe to WebSocket notifications** (recommended)
2. **Poll the API for new transactions**

#### WebSocket Subscription

```javascript
const WebSocket = require('ws');
const ws = new WebSocket('wss://node.supernova.io/api/v1/ws');

ws.on('open', function open() {
  ws.send(JSON.stringify({
    type: 'subscribe',
    channel: 'address_transactions',
    addresses: ['sn1qkl4hvc5zw8ru6tpzj2lf0u4s5jx9rgwep2j5d5']
  }));
});

ws.on('message', function incoming(data) {
  const message = JSON.parse(data);
  if (message.type === 'transaction') {
    console.log('New transaction:', message.data);
    // Process deposit
  }
});
```

#### Polling API

```javascript
async function checkForDeposits() {
  const response = await fetch('https://node.supernova.io/api/v1/wallet/transactions?limit=10', {
    headers: {
      'Authorization': 'Bearer YOUR_API_KEY'
    }
  });
  
  const data = await response.json();
  
  if (data.success) {
    // Process new transactions
    for (const tx of data.data.transactions) {
      if (tx.category === 'receive' && tx.confirmations >= 6) {
        console.log('Confirmed deposit:', tx);
        // Process deposit
      }
    }
  }
}

// Poll every minute
setInterval(checkForDeposits, 60000);
```

### Processing Withdrawals

To process withdrawals, create and send transactions:

```bash
curl -X POST "https://node.supernova.io/api/v1/wallet/send" \
  -H "Authorization: Bearer YOUR_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{
    "outputs": [
      {
        "address": "sn1q9h6s5m7p4euj6h9g5tc9qxt6azqzs9t4f5nc59",
        "amount": 5000000
      }
    ],
    "fee_rate": 10,
    "subtract_fee_from_amount": false,
    "replaceable": true,
    "comment": "Customer withdrawal"
  }'
```

Response:
```json
{
  "success": true,
  "data": {
    "txid": "7f2c3a7e4d5b6c8d9e0f1a2b3c4d5e6f7a8b9c0d1e2f3a4b5c6d7e8f9a0b1c2d",
    "fee": 1420,
    "size": 142,
    "inputs": [
      {
        "txid": "a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0c1d2e3f4a5b6c7d8e9f0a1b2",
        "vout": 1,
        "amount": 7000000
      }
    ],
    "outputs": [
      {
        "address": "sn1q9h6s5m7p4euj6h9g5tc9qxt6azqzs9t4f5nc59",
        "amount": 5000000
      },
      {
        "address": "sn1qvs9tr4j9ytc4hjruhx8mqz2ezkfq2gj6ep9ayp",
        "amount": 1998580,
        "is_change": true
      }
    ],
    "raw_tx": "0100000001c3b2a1f0e9f8d7c6b5a4f3e2d1c0b9a8f7e6d5c4b3a2f1e0d9c8b7a6f5e4d3c2010000006a47304402203a87fd3cbbb45d...",
    "carbon_footprint": {
      "emissions_gCO2": 0.28,
      "energy_consumption_kWh": 0.00082
    }
  }
}
```

## Advanced Features

### Quantum-resistant Signatures

SuperNova supports two post-quantum signature schemes:

1. **Dilithium** - For standard transactions
2. **Falcon** - For applications requiring smaller signatures

To specify the quantum signature scheme when creating a transaction:

```json
{
  "outputs": [...],
  "quantum_signature": {
    "scheme": "dilithium",
    "strength": "medium"  // "low", "medium", or "high"
  }
}
```

### Environmental Impact

SuperNova includes comprehensive environmental impact tracking:

```bash
curl -X GET "https://node.supernova.io/api/v1/stats/environmental" \
  -H "Authorization: Bearer YOUR_API_KEY"
```

To show users transaction carbon footprint:

```javascript
async function getTransactionFootprint(txid) {
  const response = await fetch(`https://node.supernova.io/api/v1/wallet/transactions/${txid}`, {
    headers: {
      'Authorization': 'Bearer YOUR_API_KEY'
    }
  });
  
  const data = await response.json();
  
  if (data.success) {
    return {
      emissions_gCO2: data.data.carbon_footprint.emissions_gCO2,
      energy_consumption_kWh: data.data.carbon_footprint.energy_consumption_kWh
    };
  }
  
  return null;
}
```

### Lightning Network

SuperNova includes full Lightning Network support for fast, off-chain transactions:

```bash
# Create a lightning invoice
curl -X POST "https://node.supernova.io/api/v1/wallet/lightning/create-invoice" \
  -H "Authorization: Bearer YOUR_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{
    "amount": 100000,
    "description": "Coffee payment"
  }'
```

Response:
```json
{
  "success": true,
  "data": {
    "payment_request": "lnsn1qcp4256yw...",
    "created_at": "2025-04-15T12:34:56Z",
    "expires_at": "2025-04-15T13:34:56Z",
    "amount": 100000,
    "description": "Coffee payment",
    "payment_hash": "a1b2c3d4e5f6...",
    "carbon_footprint": {
      "emissions_gCO2": 0.0002,
      "energy_consumption_kWh": 0.0000006
    }
  }
}
```

## Security Considerations

### Key Management

- **Cold Storage**: Keep majority of funds in cold storage
- **Hot Wallet**: Maintain minimal balance for daily operations
- **Multi-signature**: Use multi-signature wallets for hot storage
- **Key Backups**: Securely store encrypted key backups

### Transaction Security

- **Confirmations**: Wait for 6 confirmations for large deposits
- **Address Validation**: Implement client-side address validation
- **Double-Spend Protection**: Monitor for conflicting transactions
- **Fee Rate**: Use appropriate fee rates based on network congestion

### API Security

- **Rate Limiting**: Implement client-side rate limiting
- **Error Handling**: Properly handle API errors
- **Idempotency**: Use idempotency keys for withdrawal requests
- **Monitoring**: Set up alerts for unusual API behavior

## Common Integration Patterns

### Exchange Integration

1. **Address Generation** - Generate unique deposit addresses for customers
2. **Deposit Processing** - Monitor addresses for incoming transactions
3. **Confirmation Tracking** - Track confirmations and update customer balances
4. **Withdrawal Processing** - Process withdrawal requests securely
5. **Hot/Cold Wallet Management** - Implement secure wallet architecture

### Merchant Integration

1. **Invoice Generation** - Create payment requests with QR codes
2. **Payment Detection** - Monitor for incoming payments
3. **Order Fulfillment** - Process orders upon payment confirmation
4. **Refund Processing** - Handle refunds when necessary

## Testing

SuperNova provides a testnet environment for integration testing:

```toml
[node]
chain_id = "supernova-testnet"
```

### Test Faucet

Obtain testnet tokens from the faucet:

```bash
curl -X POST "https://faucet.testnet.supernovanetwork.xyz/request" \
  -H "Content-Type: application/json" \
  -d '{
    "address": "snt1qkl4hvc5zw8ru6tpzj2lf0u4s5jx9rgwep2j5d5",
    "amount": 1000000
  }'
```

### Integration Testing

Test the following aspects:

1. **Address Generation** - Verify address format and compatibility
2. **Deposit Detection** - Test deposit notifications
3. **Transaction Construction** - Verify transaction creation
4. **Confirmation Handling** - Test confirmation tracking
5. **Error Scenarios** - Test error handling and recovery

## Support

For integration support, contact:

- **Documentation**: [https://docs.supernovanetwork.xyz](https://docs.supernova.io)
- **Discord**: [SuperNova Developers](https://discord.gg/supernova-dev)
- **GitHub**: [SuperNova Issues](https://github.com/username/supernova/issues)
- **Email**: integration-support@supernova.io

## Appendix

### Sample Code

For sample integration code in various languages, refer to the [examples](https://github.com/username/supernova-examples) repository.

### API Libraries

- **JavaScript**: [`supernova-js`](https://github.com/username/supernova-js)
- **Python**: [`supernova-py`](https://github.com/username/supernova-py)
- **Go**: [`supernova-go`](https://github.com/username/supernova-go)
- **Java**: [`supernova-java`](https://github.com/username/supernova-java)
- **PHP**: [`supernova-php`](https://github.com/username/supernova-php) 
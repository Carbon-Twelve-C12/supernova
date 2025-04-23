# Wallet API Reference

This document describes the Wallet API endpoints provided by the SuperNova blockchain node.

## Endpoints

All wallet API endpoints are prefixed with `/api/v1/wallet`.

## Authentication

Some wallet endpoints require authentication. To authenticate, include an API key in the `Authorization` header:

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

## Wallet Endpoints

### GET /api/v1/wallet/info

Get information about the wallet.

**Parameters:** None

**Response:**
```json
{
  "success": true,
  "data": {
    "wallet_id": "primary",
    "wallet_type": "HD",
    "balance": 10000000,
    "unconfirmed_balance": 500000,
    "address_count": 3,
    "tx_count": 25,
    "last_active": "2025-04-15T12:34:56Z",
    "hd_master_key_fingerprint": "ab12cd34",
    "is_locked": false
  }
}
```

### GET /api/v1/wallet/balance

Get wallet balance information.

**Parameters:** None

**Response:**
```json
{
  "success": true,
  "data": {
    "confirmed": 10000000,
    "unconfirmed": 500000,
    "immature": 0,
    "total": 10500000,
    "spendable": 10500000,
    "pending_rewards": 0
  }
}
```

### GET /api/v1/wallet/addresses

List wallet addresses.

**Parameters:**
- `active_only` (optional): If set to `true`, returns only active addresses (default: `false`)
- `limit` (optional): Number of addresses to return (default: `100`)
- `offset` (optional): Pagination offset (default: `0`)

**Response:**
```json
{
  "success": true,
  "data": {
    "addresses": [
      {
        "address": "sn1q9h6s5m7p4euj6h9g5tc9qxt6azqzs9t4f5nc59",
        "type": "receive",
        "hd_path": "m/84'/0'/0'/0/0",
        "balance": 5000000,
        "tx_count": 10,
        "label": "Main address",
        "last_used": "2025-04-10T08:45:12Z"
      },
      {
        "address": "sn1qvs9tr4j9ytc4hjruhx8mqz2ezkfq2gj6ep9ayp",
        "type": "change",
        "hd_path": "m/84'/0'/0'/1/0",
        "balance": 5000000,
        "tx_count": 15,
        "label": null,
        "last_used": "2025-04-15T12:34:56Z"
      }
    ],
    "total_count": 2
  }
}
```

### GET /api/v1/wallet/addresses/{address}

Get information about a specific address.

**Parameters:**
- `address` (path): The wallet address to query

**Response:**
```json
{
  "success": true,
  "data": {
    "address": "sn1q9h6s5m7p4euj6h9g5tc9qxt6azqzs9t4f5nc59",
    "type": "receive",
    "hd_path": "m/84'/0'/0'/0/0",
    "balance": 5000000,
    "tx_count": 10,
    "label": "Main address",
    "last_used": "2025-04-10T08:45:12Z",
    "utxos": [
      {
        "txid": "7f2c3a7e4d5b6c8d9e0f1a2b3c4d5e6f7a8b9c0d1e2f3a4b5c6d7e8f9a0b1c2d",
        "vout": 0,
        "amount": 3000000,
        "confirmations": 120,
        "height": 5000,
        "spendable": true
      },
      {
        "txid": "a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0c1d2e3f4a5b6c7d8e9f0a1b2",
        "vout": 1,
        "amount": 2000000,
        "confirmations": 80,
        "height": 5040,
        "spendable": true
      }
    ]
  }
}
```

### POST /api/v1/wallet/addresses/new

Generate a new address.

**Request Body:**
```json
{
  "label": "My new address",
  "type": "receive"
}
```

**Response:**
```json
{
  "success": true,
  "data": {
    "address": "sn1qkl4hvc5zw8ru6tpzj2lf0u4s5jx9rgwep2j5d5",
    "type": "receive",
    "hd_path": "m/84'/0'/0'/0/1",
    "label": "My new address"
  }
}
```

### GET /api/v1/wallet/transactions

List wallet transactions.

**Parameters:**
- `limit` (optional): Number of transactions to return (default: `50`)
- `offset` (optional): Pagination offset (default: `0`)
- `include_raw` (optional): Include raw transaction data (default: `false`)
- `category` (optional): Filter by category (send, receive, generate, immature, fee)
- `start_time` (optional): Filter by start time (ISO format)
- `end_time` (optional): Filter by end time (ISO format)

**Response:**
```json
{
  "success": true,
  "data": {
    "transactions": [
      {
        "txid": "7f2c3a7e4d5b6c8d9e0f1a2b3c4d5e6f7a8b9c0d1e2f3a4b5c6d7e8f9a0b1c2d",
        "time": "2025-04-15T12:34:56Z",
        "amount": 3000000,
        "fee": 1000,
        "confirmations": 120,
        "height": 5000,
        "blockhash": "a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0c1d2e3f4a5b6c7d8e9f0a1b2",
        "category": "receive",
        "address": "sn1q9h6s5m7p4euj6h9g5tc9qxt6azqzs9t4f5nc59",
        "label": "Main address",
        "inputs": [...],
        "outputs": [...],
        "carbon_footprint": {
          "emissions_gCO2": 0.25,
          "energy_consumption_kWh": 0.00075
        }
      }
    ],
    "total_count": 25
  }
}
```

### GET /api/v1/wallet/transactions/{txid}

Get information about a specific transaction.

**Parameters:**
- `txid` (path): The transaction ID
- `include_raw` (optional): Include raw transaction data (default: `false`)

**Response:**
```json
{
  "success": true,
  "data": {
    "txid": "7f2c3a7e4d5b6c8d9e0f1a2b3c4d5e6f7a8b9c0d1e2f3a4b5c6d7e8f9a0b1c2d",
    "time": "2025-04-15T12:34:56Z",
    "amount": 3000000,
    "fee": 1000,
    "confirmations": 120,
    "height": 5000,
    "blockhash": "a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0c1d2e3f4a5b6c7d8e9f0a1b2",
    "category": "receive",
    "address": "sn1q9h6s5m7p4euj6h9g5tc9qxt6azqzs9t4f5nc59",
    "label": "Main address",
    "inputs": [
      {
        "txid": "b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0c1d2e3f4a5b6c7d8e9f0a1b2c3",
        "vout": 1,
        "address": "sn1qxy7rzj86q77k8r9cnxd7tghvsslk7jdz2sjgdz",
        "amount": 4000000
      }
    ],
    "outputs": [
      {
        "address": "sn1q9h6s5m7p4euj6h9g5tc9qxt6azqzs9t4f5nc59",
        "amount": 3000000,
        "n": 0,
        "is_change": false
      },
      {
        "address": "sn1qxy7rzj86q77k8r9cnxd7tghvsslk7jdz2sjgdz",
        "amount": 999000,
        "n": 1,
        "is_change": true
      }
    ],
    "carbon_footprint": {
      "emissions_gCO2": 0.25,
      "energy_consumption_kWh": 0.00075
    },
    "raw": "0100000001c3b2a1f0e9f8d7c6b5a4f3e2d1c0b9a8f7e6d5c4b3a2f1e0d9c8b7a6f5e4d3c2010000006a47304402203a87fd3cbbb45d..." // Only if include_raw=true
  }
}
```

### GET /api/v1/wallet/utxos

List unspent transaction outputs (UTXOs).

**Parameters:**
- `min_conf` (optional): Minimum confirmations (default: `1`)
- `max_conf` (optional): Maximum confirmations (default: `9999999`)
- `min_amount` (optional): Minimum amount (default: `0`)
- `max_amount` (optional): Maximum amount (default: unlimited)
- `include_unsafe` (optional): Include outputs that are not safe to spend (default: `false`)

**Response:**
```json
{
  "success": true,
  "data": {
    "utxos": [
      {
        "txid": "7f2c3a7e4d5b6c8d9e0f1a2b3c4d5e6f7a8b9c0d1e2f3a4b5c6d7e8f9a0b1c2d",
        "vout": 0,
        "address": "sn1q9h6s5m7p4euj6h9g5tc9qxt6azqzs9t4f5nc59",
        "amount": 3000000,
        "confirmations": 120,
        "height": 5000,
        "spendable": true,
        "safe": true
      },
      {
        "txid": "a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0c1d2e3f4a5b6c7d8e9f0a1b2",
        "vout": 1,
        "address": "sn1qvs9tr4j9ytc4hjruhx8mqz2ezkfq2gj6ep9ayp",
        "amount": 7000000,
        "confirmations": 80,
        "height": 5040,
        "spendable": true,
        "safe": true
      }
    ],
    "total_amount": 10000000
  }
}
```

### POST /api/v1/wallet/send

Send a transaction.

**Request Body:**
```json
{
  "outputs": [
    {
      "address": "sn1q9h6s5m7p4euj6h9g5tc9qxt6azqzs9t4f5nc59",
      "amount": 5000000
    }
  ],
  "fee_rate": 10,     // satoshis per byte
  "subtract_fee_from_amount": false,
  "replaceable": true,
  "comment": "Payment for services"
}
```

**Response:**
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

### POST /api/v1/wallet/sign

Sign a message or transaction.

**Request Body:**
```json
{
  "type": "message",  // or "transaction"
  "data": "Message to sign",  // or raw transaction hex
  "address": "sn1q9h6s5m7p4euj6h9g5tc9qxt6azqzs9t4f5nc59"  // if signing a message
}
```

**Response:**
```json
{
  "success": true,
  "data": {
    "signature": "H5Dpt3KJU/kLePILhkqR4fMB9L1UfbXMm3BdXNToj1vk7qbzHZMtWvGpD/vuvPHwZfTGKLb4P9U5qfNxj2mPH6c=",
    "address": "sn1q9h6s5m7p4euj6h9g5tc9qxt6azqzs9t4f5nc59",
    "type": "message"
  }
}
```

### POST /api/v1/wallet/verify

Verify a message signature.

**Request Body:**
```json
{
  "message": "Message to verify",
  "signature": "H5Dpt3KJU/kLePILhkqR4fMB9L1UfbXMm3BdXNToj1vk7qbzHZMtWvGpD/vuvPHwZfTGKLb4P9U5qfNxj2mPH6c=",
  "address": "sn1q9h6s5m7p4euj6h9g5tc9qxt6azqzs9t4f5nc59"
}
```

**Response:**
```json
{
  "success": true,
  "data": {
    "valid": true,
    "address": "sn1q9h6s5m7p4euj6h9g5tc9qxt6azqzs9t4f5nc59"
  }
}
```

### POST /api/v1/wallet/label

Set a label for an address.

**Request Body:**
```json
{
  "address": "sn1q9h6s5m7p4euj6h9g5tc9qxt6azqzs9t4f5nc59",
  "label": "New label"
}
```

**Response:**
```json
{
  "success": true,
  "data": {
    "address": "sn1q9h6s5m7p4euj6h9g5tc9qxt6azqzs9t4f5nc59",
    "label": "New label"
  }
}
```

### POST /api/v1/wallet/lock

Lock the wallet.

**Parameters:** None

**Response:**
```json
{
  "success": true,
  "data": {
    "locked": true
  }
}
```

### POST /api/v1/wallet/unlock

Unlock the wallet.

**Request Body:**
```json
{
  "passphrase": "your wallet passphrase",
  "timeout": 300  // Lock after 300 seconds
}
```

**Response:**
```json
{
  "success": true,
  "data": {
    "unlocked": true,
    "timeout": 300
  }
}
```

### POST /api/v1/wallet/backup

Create a wallet backup.

**Parameters:** None

**Response:**
```json
{
  "success": true,
  "data": {
    "backup_file": "/path/to/wallet-backup-2025-04-15-123456.dat",
    "timestamp": "2025-04-15T12:34:56Z",
    "checksum": "a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6"
  }
}
```

## Error Codes

- `400` - Bad Request: Invalid parameters or request
- `401` - Unauthorized: Authentication required
- `403` - Forbidden: Insufficient permissions
- `404` - Not Found: Resource not found
- `500` - Internal Server Error: Server error

## Examples

### Curl Examples

#### Get Wallet Info
```bash
curl -X GET "https://node.supernova.io/api/v1/wallet/info" \
  -H "Authorization: Bearer YOUR_API_KEY"
```

#### Create a New Address
```bash
curl -X POST "https://node.supernova.io/api/v1/wallet/addresses/new" \
  -H "Authorization: Bearer YOUR_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{
    "label": "My new address",
    "type": "receive"
  }'
```

#### Send a Transaction
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
    "comment": "Payment for services"
  }'
``` 
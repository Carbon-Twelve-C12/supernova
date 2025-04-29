# API Documentation

## Overview

Supernova provides a comprehensive API system that enables developers to interact with the blockchain network programmatically. This document provides an overview of the available APIs, authentication methods, and usage guidelines. The Supernova API system is designed to be developer-friendly, well-documented, and secure.

## API Types

Supernova offers multiple API interfaces to accommodate different developer needs:

### 1. RESTful API

The RESTful API follows modern REST principles with JSON responses. This API is ideal for web applications, mobile apps, and integration with contemporary development stacks.

- **Base URL**: `https://api.supernovanetwork.xyz/v1`
- **Format**: JSON
- **Methods**: GET, POST, PUT, DELETE
- **Status Codes**: Standard HTTP status codes

### 2. JSON-RPC API

The JSON-RPC API provides a Bitcoin-compatible interface following the JSON-RPC 2.0 specification. This API is ideal for existing blockchain applications and tools that already support Bitcoin-like APIs.

- **Default Endpoint**: `https://api.supernovanetwork.xyz/json-rpc`
- **Format**: JSON-RPC 2.0
- **Transport**: HTTP, WebSockets

### 3. WebSocket API

The WebSocket API enables real-time data streaming and subscription-based updates. This API is ideal for applications requiring live updates without polling.

- **Endpoint**: `wss://api.supernovanetwork.xyz/ws`
- **Subscription Model**: Topic-based subscriptions
- **Message Format**: JSON

### 4. GraphQL API

The GraphQL API allows flexible, client-specified queries in a single request. This API is ideal for applications with complex or variable data requirements.

- **Endpoint**: `https://api.supernovanetwork.xyz/graphql`
- **Queries**: Custom query construction
- **Documentation**: Built-in schema explorer

## Authentication Methods

Supernova supports multiple authentication methods to balance security with ease of use:

### API Keys

- **Header**: `X-API-Key: your-api-key`
- **Creation**: Generated through the developer portal
- **Management**: Keys can be revoked, regenerated, and assigned specific permissions
- **Rate Limits**: Tiered based on API key level

### JWT Authentication

- **Header**: `Authorization: Bearer your-jwt-token`
- **Flow**: Obtain token via `/auth` endpoint with credentials
- **Expiration**: Tokens expire after a configurable period
- **Refresh**: Tokens can be refreshed without re-authenticating

### OAuth 2.0

- **Supported Flows**: Authorization Code, Client Credentials
- **Providers**: Support for integration with external identity providers
- **Scopes**: Fine-grained permission control
- **PKCE Support**: Enhanced security for public clients

## Rate Limiting

API access is subject to rate limiting to ensure fair usage and system stability:

| Tier | Requests per Minute | Concurrent Connections |
|------|---------------------|------------------------|
| Basic | 60 | 5 |
| Pro | 300 | 15 |
| Enterprise | 1,000+ | 50+ |

Rate limit headers are included in all responses:
- `X-RateLimit-Limit`: Total requests allowed
- `X-RateLimit-Remaining`: Requests remaining in the current window
- `X-RateLimit-Reset`: Time when the limit resets

## API Categories

### Blockchain Data APIs

APIs for accessing core blockchain data:

- **Blocks**: Retrieve block data, headers, and chain information
- **Transactions**: Access transaction details, confirmations, and history
- **Addresses**: Query address balances, UTXOs, and activity
- **Mempool**: View pending transactions and fee estimates

### Wallet APIs

APIs for wallet functionality:

- **Account Management**: Create and manage wallet accounts
- **Transactions**: Send transactions, estimate fees, manage UTXOs
- **Keys and Addresses**: Generate addresses, manage keys
- **Signatures**: Create and verify signatures

### Network APIs

APIs for interacting with the network:

- **Peers**: View and manage peer connections
- **Network Status**: Monitor network health and statistics
- **Propagation**: Broadcast transactions and data
- **Network Parameters**: View current network configuration

### Environmental APIs

APIs for environmental impact tracking:

- **Energy Metrics**: Access energy consumption data
- **Carbon Footprint**: Calculate carbon emissions
- **Green Mining**: Verify renewable energy claims
- **Environmental Treasury**: Track environmental funds

### Advanced APIs

Specialized APIs for advanced use cases:

- **Lightning Network**: Manage payment channels and route payments
- **Smart Contracts**: Interact with Supernova's smart contract system
- **Quantum-Resistant Features**: Access post-quantum cryptographic functions
- **Analytics**: Advanced blockchain analytics and metrics

## RESTful API Reference

### Base URL

All RESTful API endpoints are available at:

```
https://api.supernovanetwork.xyz/v1
```

### Common Endpoints

#### Blockchain

```
GET /blocks                    # List blocks
GET /blocks/latest             # Get latest block
GET /blocks/{hash}             # Get block by hash
GET /blocks/height/{height}    # Get block by height
GET /transactions/{txid}       # Get transaction by ID
GET /addresses/{address}       # Get address information
```

#### Wallet

```
GET /wallet/balance            # Get wallet balance
POST /wallet/send              # Send transaction
GET /wallet/transactions       # List wallet transactions
GET /wallet/utxos              # List unspent transaction outputs
POST /wallet/addresses/new     # Generate new address
```

#### Network

```
GET /network/info              # Get network information
GET /network/peers             # List connected peers
GET /network/mempool           # Get mempool information
GET /network/fee-estimates     # Get fee estimates
```

#### Environmental

```
GET /environment/metrics               # Get network environmental metrics
GET /environment/transaction/{txid}    # Get transaction environmental impact
GET /environment/blocks/{hash}         # Get block environmental data
GET /environment/mining/green          # Get green mining information
```

### Example Request and Response

Request:
```bash
curl -X GET "https://api.supernovanetwork.xyz/v1/blocks/latest" \
  -H "X-API-Key: your-api-key"
```

Response:
```json
{
  "hash": "000000000000000000024bead8df69990852c202db0e0097c1a12ea637d7e96d",
  "height": 54321,
  "version": 2,
  "timestamp": 1636243812,
  "tx_count": 1345,
  "size": 1256423,
  "merkle_root": "4a5e1e4baab89f3a32518a88c31bc87f618f76673e2cc77ab2127b7afdeda33b",
  "previous_block_hash": "0000000000000000000a1a8c4a7c598ec5cd53c51f74b79b3828c2b0bfab3ac6",
  "bits": "1d00ffff",
  "nonce": 2083236893,
  "environmental_data": {
    "energy_consumption_kwh": 12.45,
    "carbon_emissions_kg": 5.28,
    "renewable_percentage": 65
  }
}
```

## JSON-RPC API Reference

### Endpoint

The JSON-RPC API is available at:

```
https://api.supernovanetwork.xyz/json-rpc
```

### Request Format

```json
{
  "jsonrpc": "2.0",
  "id": "request-id",
  "method": "method-name",
  "params": [param1, param2]
}
```

### Common Methods

#### Blockchain

```
getblock                # Get block by hash
getblockchaininfo       # Get blockchain information
getblockhash            # Get block hash at height
gettransaction          # Get transaction details
getbalance              # Get address balance
```

#### Wallet

```
createwallet            # Create a new wallet
getwalletinfo           # Get wallet information
listunspent             # List unspent outputs
sendtoaddress           # Send to address
signmessage             # Sign a message
```

#### Network

```
getnetworkinfo          # Get network information
getpeerinfo             # Get connected peer information
getmempoolinfo          # Get mempool information
estimatefee             # Estimate transaction fee
```

#### Environmental

```
getenvironmentalinfo           # Get network environmental metrics
gettransactionenvironmental    # Get transaction environmental impact
getblockenvironmental          # Get block environmental data
getgreenmining                 # Get green mining information
```

### Example Request and Response

Request:
```bash
curl -X POST "https://api.supernovanetwork.xyz/json-rpc" \
  -H "Content-Type: application/json" \
  -H "X-API-Key: your-api-key" \
  -d '{
    "jsonrpc": "2.0",
    "id": "1",
    "method": "getblockchaininfo",
    "params": []
  }'
```

Response:
```json
{
  "jsonrpc": "2.0",
  "id": "1",
  "result": {
    "chain": "main",
    "blocks": 54321,
    "headers": 54321,
    "bestblockhash": "000000000000000000024bead8df69990852c202db0e0097c1a12ea637d7e96d",
    "difficulty": 21448277761059.71,
    "mediantime": 1636243100,
    "verificationprogress": 0.9999753214994448,
    "pruned": false,
    "environmental": {
      "total_energy_kwh": 583723.45,
      "renewable_percentage": 62,
      "carbon_emissions_kg": 128456.23
    }
  }
}
```

## WebSocket API Reference

### Connection

Connect to the WebSocket API at:

```
wss://api.supernovanetwork.xyz/ws
```

### Authentication

Authentication is performed on connection via:
- URL query parameter: `?api_key=your-api-key`
- Authentication message after connection

### Subscription Model

Subscribe to topics to receive real-time updates:

```json
{
  "op": "subscribe",
  "topics": ["blocks", "mempool", "address/1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa"]
}
```

### Available Topics

- `blocks`: New block notifications
- `transactions`: New transaction notifications
- `mempool`: Mempool updates
- `address/{address}`: Address activity
- `environmental`: Environmental metric updates

### Example Messages

Subscribe:
```json
{
  "op": "subscribe",
  "topics": ["blocks"]
}
```

Received Block Notification:
```json
{
  "topic": "blocks",
  "data": {
    "hash": "000000000000000000024bead8df69990852c202db0e0097c1a12ea637d7e96d",
    "height": 54321,
    "tx_count": 1345,
    "timestamp": 1636243812
  }
}
```

## GraphQL API Reference

### Endpoint

The GraphQL API is available at:

```
https://api.supernovanetwork.xyz/graphql
```

### Schema Explorer

Interactive schema exploration is available at:

```
https://api.supernovanetwork.xyz/graphql/explorer
```

### Example Queries

#### Get Block with Transactions

```graphql
query {
  block(hash: "000000000000000000024bead8df69990852c202db0e0097c1a12ea637d7e96d") {
    hash
    height
    timestamp
    transactions {
      txid
      fee
      inputs {
        prevout
        value
      }
      outputs {
        address
        value
      }
    }
    environmental {
      energyConsumptionKwh
      carbonEmissionsKg
      renewablePercentage
    }
  }
}
```

#### Get Address with Balance and Transactions

```graphql
query {
  address(hash: "1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa") {
    hash
    balance
    totalReceived
    totalSent
    txCount
    transactions(limit: 10) {
      txid
      timestamp
      value
      confirmations
    }
  }
}
```

## SDKs and Client Libraries

Supernova provides official client libraries for popular programming languages:

### JavaScript/TypeScript SDK

```javascript
import { SupernovaClient } from 'supernova-sdk';

const client = new SupernovaClient({
  apiKey: 'your-api-key',
  network: 'mainnet'
});

async function getLatestBlock() {
  const block = await client.blockchain.getLatestBlock();
  console.log(block);
}
```

### Python SDK

```python
from supernova_sdk import SupernovaClient

client = SupernovaClient(
  api_key='your-api-key',
  network='mainnet'
)

def get_latest_block():
  block = client.blockchain.get_latest_block()
  print(block)
```

### Java SDK

```java
import network.supernova.sdk.SupernovaClient;

SupernovaClient client = SupernovaClient.builder()
  .apiKey("your-api-key")
  .network("mainnet")
  .build();

public void getLatestBlock() {
  Block block = client.blockchain().getLatestBlock();
  System.out.println(block);
}
```

### Go SDK

```go
package main

import (
  "fmt"
  "github.com/mjohnson518/supernova-sdk-go"
)

func main() {
  client := supernova.NewClient(supernova.Config{
    APIKey: "your-api-key",
    Network: "mainnet",
  })
  
  block, err := client.Blockchain.GetLatestBlock()
  if err != nil {
    panic(err)
  }
  
  fmt.Println(block)
}
```

## API Versioning

Supernova follows semantic versioning for API endpoints:

- **Major Version** (`v1`, `v2`): Breaking changes, in the URL path
- **Minor Version**: Non-breaking additions, in the `X-API-Version` header
- **Patch Version**: Bug fixes, not exposed in the API

API versions have the following lifecycle:
1. **Beta**: Early testing, subject to change
2. **Stable**: Fully supported
3. **Deprecated**: Schedule for removal (minimum 6 months notice)
4. **Sunset**: No longer available

## Rate Limiting Strategies

### Best Practices

1. **Use Bulk Operations**: Combine multiple operations in a single request
2. **Implement Caching**: Cache responses that don't change frequently
3. **Use WebSockets**: Subscribe to updates instead of polling
4. **Handle Rate Limit Errors**: Implement exponential backoff
5. **Monitor Usage**: Track your API usage metrics

### Handling Rate Limiting

When rate limited, APIs return HTTP 429 (Too Many Requests) with headers:
- `Retry-After`: Seconds to wait before retrying
- `X-RateLimit-Reset`: Timestamp when the limit resets

## Error Handling

### RESTful API Error Format

```json
{
  "error": {
    "code": "insufficient_funds",
    "message": "Wallet has insufficient funds for this transaction",
    "details": {
      "required": 1.5,
      "available": 1.2
    },
    "request_id": "f7cbb3a2-1e74-42b7-a302-b5635516e1a2"
  }
}
```

### JSON-RPC Error Format

```json
{
  "jsonrpc": "2.0",
  "id": "1",
  "error": {
    "code": -32000,
    "message": "Insufficient funds",
    "data": {
      "required": 1.5,
      "available": 1.2,
      "request_id": "f7cbb3a2-1e74-42b7-a302-b5635516e1a2"
    }
  }
}
```

### Common Error Codes

| HTTP Status | JSON-RPC Code | Description |
|-------------|---------------|-------------|
| 400 | -32600 | Invalid request |
| 401 | -32800 | Unauthorized |
| 403 | -32801 | Forbidden |
| 404 | -32601 | Method not found |
| 422 | -32602 | Invalid parameters |
| 429 | -32000 | Rate limit exceeded |
| 500 | -32603 | Internal error |

## Webhooks

Supernova supports webhooks for event-based notifications:

### Registration

```bash
curl -X POST "https://api.supernovanetwork.xyz/v1/webhooks" \
  -H "X-API-Key: your-api-key" \
  -H "Content-Type: application/json" \
  -d '{
    "url": "https://your-server.com/supernova-webhook",
    "events": ["block.new", "tx.confirmed", "address.received"],
    "address_filters": ["1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa"]
  }'
```

### Event Types

- `block.new`: New block mined
- `tx.new`: New transaction in mempool
- `tx.confirmed`: Transaction confirmed
- `address.received`: Address received funds
- `address.spent`: Address spent funds
- `environmental.update`: Environmental metrics updated

### Webhook Payload

```json
{
  "id": "whk_7H98s7d9HJ9h79",
  "event": "tx.confirmed",
  "created_at": "2023-07-15T12:34:56Z",
  "data": {
    "txid": "4a5e1e4baab89f3a32518a88c31bc87f618f76673e2cc77ab2127b7afdeda33b",
    "block_height": 54321,
    "confirmations": 1,
    "fee": 0.00021,
    "size": 225,
    "inputs": [...],
    "outputs": [...]
  }
}
```

## Conclusion

The Supernova API system provides a comprehensive and flexible interface for developers to integrate with the Supernova blockchain network. Whether you need simple REST endpoints, traditional JSON-RPC methods, real-time WebSocket updates, or customized GraphQL queries, Supernova's API suite has you covered.

For detailed information about specific endpoints, parameters, and response formats, please refer to the individual API reference documentation sections. 
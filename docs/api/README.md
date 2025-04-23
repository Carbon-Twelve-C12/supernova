# SuperNova API Documentation

The SuperNova API provides a comprehensive interface for interacting with the SuperNova blockchain node, including functionality for blockchain data access, mempool management, network operations, mining, environmental metrics, Lightning Network operations, and node management.

## API Structure

The API is organized into the following modules:

- **Blockchain API**: Access blockchain data including blocks and transactions
- **Mempool API**: View and manage pending transactions
- **Network API**: Interact with and monitor the P2P network
- **Mining API**: Control mining operations and retrieve mining information
- **Environmental API**: Track and manage environmental impact metrics
- **Lightning API**: Interact with the Lightning Network for off-chain payments
- **Node API**: Manage and monitor the node itself

## API Versioning

All API endpoints are versioned with a prefix: `/api/v1/...`. Future API versions will use different version prefixes.

## Authentication

Some API endpoints may require authentication, which is done via JWT tokens in the `Authorization` header.

## Response Format

All API responses are JSON-formatted and include appropriate HTTP status codes. Success responses typically include the requested data, while error responses include an error message and code.

## OpenAPI Documentation

Interactive API documentation is available at the `/api-docs` endpoint of your running node, which provides a SwaggerUI interface for exploring and testing the API.

## Rate Limiting

API requests may be subject to rate limiting to ensure service stability. Rate limit information is provided in response headers.

## Detailed Documentation

- [Blockchain API](blockchain.md) - Access blocks and transactions
- [Mempool API](mempool.md) - View and manage memory pool transactions
- [Network API](network.md) - Peer-to-peer network operations
- [Mining API](mining.md) - Mining operations and controls
- [Environmental API](environmental.md) - Environmental impact tracking
- [Lightning API](lightning.md) - Lightning Network payment channels
- [Node API](node.md) - Node management and monitoring

## Example Usage

Here's a basic example of how to retrieve blockchain information using curl:

```bash
curl -X GET http://node-address:port/api/v1/blockchain/info
```

Example response:

```json
{
  "chain": "main",
  "blocks": 750000,
  "headers": 750000,
  "bestblockhash": "000000000000000000034bdb3c01621a0c84787efafd807aa104c9a327537a39",
  "difficulty": 29337160522857.9,
  "mediantime": 1658180961,
  "verificationprogress": 0.999999,
  "pruned": false,
  "softforks": {
    "taproot": {
      "status": "active",
      "height": 709632
    }
  },
  "size_on_disk": 486932398499
}
``` 
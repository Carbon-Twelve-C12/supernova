# Supernova Network Protocol Specification

## Overview

This document specifies the peer-to-peer network protocol used by Supernova nodes to communicate with each other. The protocol is based on libp2p with custom extensions for blockchain-specific functionality.

**Version:** 1.0
**Last Updated:** December 2025

---

## Protocol Stack

```
┌─────────────────────────────────────┐
│         Application Layer           │
│  (Blocks, Transactions, Consensus)  │
├─────────────────────────────────────┤
│        Message Layer (Gossipsub)    │
│    (Topic-based pub/sub messaging)  │
├─────────────────────────────────────┤
│         Request/Response            │
│      (Direct peer queries)          │
├─────────────────────────────────────┤
│     Security Layer (Noise/TLS)      │
│   (Encryption, Authentication)      │
├─────────────────────────────────────┤
│       Transport Layer (TCP/QUIC)    │
│      (Reliable delivery)            │
└─────────────────────────────────────┘
```

## Transport Layer

### Supported Transports

| Transport | Port | Usage |
|-----------|------|-------|
| TCP | 8333 | Primary, reliable |
| QUIC | 8333/UDP | Low-latency, NAT-friendly |
| WebSocket | 8334 | Browser clients |

### Connection Handshake

```
1. TCP/QUIC connection established
2. Noise handshake (XX pattern)
3. Protocol negotiation (multistream-select)
4. Peer identification exchange
5. Capability advertisement
```

## Security Layer

### Encryption

All connections use the Noise Protocol Framework:
- **Pattern:** XX (mutual authentication)
- **Cipher:** ChaCha20-Poly1305
- **DH:** X25519
- **Hash:** SHA-256

### Peer Identity

Each node has a cryptographic identity:
- **Key Type:** Ed25519 (classical) or Dilithium (quantum-resistant)
- **Peer ID:** Multihash of public key
- **Format:** Base58-encoded, 46 characters

```
Example: 12D3KooWD3eckifWpRn9wQpMG9R9hX3sD6fpaKk3zBE4KPLBjhL8
```

## Message Layer (Gossipsub)

### Topics

| Topic | Content | Validation |
|-------|---------|------------|
| `/supernova/blocks/1.0.0` | New blocks | Full validation |
| `/supernova/txs/1.0.0` | Transactions | Signature + format |
| `/supernova/headers/1.0.0` | Block headers | Header validation |
| `/supernova/env/1.0.0` | Environmental data | Oracle signature |

### Message Format

```rust
struct GossipMessage {
    /// Message type identifier
    type_id: u16,
    /// Timestamp (Unix ms)
    timestamp: u64,
    /// Sender peer ID
    sender: PeerId,
    /// Message payload (protobuf-encoded)
    payload: Vec<u8>,
    /// Signature over (type_id || timestamp || payload)
    signature: Signature,
}
```

### Validation Mode

Gossipsub operates in **Strict** validation mode:
- All messages must be signed
- Invalid signatures result in peer scoring penalty
- Duplicate messages are rejected
- Out-of-order messages are queued (max 100)

### Peer Scoring

```
Score = BaseScore + TopicScores + BehaviorPenalties

TopicScore = (ValidMessages * 1.0) + (FirstDelivery * 0.5) - (InvalidMessages * 10.0)

BehaviorPenalties:
  - Invalid message: -10
  - Duplicate spam: -5
  - Slow response: -1
  - Disconnection: -2
```

Peers with score < -100 are disconnected.

## Request/Response Protocol

### Supported Requests

| Request | Response | Timeout |
|---------|----------|---------|
| `GetBlocks` | `Blocks` | 30s |
| `GetHeaders` | `Headers` | 10s |
| `GetBlockTxs` | `Transactions` | 20s |
| `GetMempool` | `MempoolTxs` | 30s |
| `Ping` | `Pong` | 5s |

### Request Format

```protobuf
message Request {
    uint32 id = 1;
    oneof request {
        GetBlocksRequest get_blocks = 2;
        GetHeadersRequest get_headers = 3;
        GetBlockTxsRequest get_block_txs = 4;
        GetMempoolRequest get_mempool = 5;
        PingRequest ping = 6;
    }
}

message GetBlocksRequest {
    // Block locator hashes (newest to oldest)
    repeated bytes locator_hashes = 1;
    // Stop hash (zero for no limit)
    bytes stop_hash = 2;
    // Maximum blocks to return
    uint32 max_blocks = 3;
}
```

### Response Format

```protobuf
message Response {
    uint32 id = 1;
    bool success = 2;
    string error = 3;
    oneof response {
        BlocksResponse blocks = 4;
        HeadersResponse headers = 5;
        TransactionsResponse transactions = 6;
        MempoolResponse mempool = 7;
        PongResponse pong = 8;
    }
}
```

## Block Propagation

### Compact Block Relay

To reduce bandwidth, blocks are propagated using compact blocks:

```protobuf
message CompactBlock {
    // Block header
    BlockHeader header = 1;
    // Short transaction IDs (6 bytes each)
    repeated bytes short_ids = 2;
    // Prefilled transactions (coinbase + predicted missing)
    repeated PrefilledTx prefilled = 3;
}

message PrefilledTx {
    uint32 index = 1;
    Transaction tx = 2;
}
```

### Block Announcement Flow

```
Node A                          Node B
   |                               |
   |--- CompactBlock announcement->|
   |                               |
   |<-- GetBlockTxs (missing) ----|
   |                               |
   |--- BlockTxs --------------->  |
   |                               |
   |                       [Validate Block]
   |                               |
   |<-- Acknowledgment -----------|
```

## Transaction Propagation

### Transaction Announcement

Transactions are announced via Gossipsub with filtering:

```protobuf
message TransactionAnnouncement {
    // Transaction ID
    bytes txid = 1;
    // Fee rate (satoshis per vbyte)
    uint64 fee_rate = 2;
    // Is quantum-resistant
    bool quantum_resistant = 3;
    // Environmental bonus factor
    float env_bonus = 4;
}
```

### Bloom Filters

Nodes may advertise bloom filters to reduce bandwidth:

```protobuf
message FilterLoad {
    bytes filter = 1;
    uint32 hash_funcs = 2;
    uint32 tweak = 3;
    uint32 flags = 4;
}
```

## Peer Discovery

### Bootstrap Nodes

Hardcoded DNS seeds:
- `seed.testnet.supernovanetwork.xyz`
- `seed2.testnet.supernovanetwork.xyz`
- `seed3.testnet.supernovanetwork.xyz`

### Kademlia DHT

Peer discovery uses Kademlia DHT:
- **Bucket size:** 20 peers
- **Alpha (parallelism):** 3
- **Refresh interval:** 1 hour

### Peer Exchange

Nodes periodically exchange known peers:

```protobuf
message PeerExchange {
    repeated PeerInfo peers = 1;
}

message PeerInfo {
    bytes peer_id = 1;
    repeated string addresses = 2;
    uint64 last_seen = 3;
}
```

## Initial Block Download (IBD)

### Sync Strategy

```
1. Connect to seed nodes
2. Request headers from genesis
3. Validate header chain
4. Download blocks in parallel (8 peers, 16 blocks each)
5. Validate blocks
6. Build UTXO set
7. Switch to normal operation
```

### Checkpoints

Hardcoded checkpoints prevent long-range attacks:

```rust
const CHECKPOINTS: &[(u64, &str)] = &[
    (0, "00000000..."),        // Genesis
    (100000, "00000000..."),   // Block 100k
    (200000, "00000000..."),   // Block 200k
    // ...
];
```

## Rate Limiting

### Per-Peer Limits

| Resource | Limit | Window |
|----------|-------|--------|
| Messages | 1000 | 10 seconds |
| Requests | 100 | 10 seconds |
| Bandwidth | 10 MB/s | Per connection |
| Connections | 125 | Total outbound |

### DoS Protection

- **Connection limits:** Max 125 outbound, 125 inbound
- **Message size limits:** 4 MB max
- **Validation timeouts:** Disconnect slow validators
- **Proof-of-work for connection:** Optional under attack

## Error Handling

### Error Codes

| Code | Name | Description |
|------|------|-------------|
| 0x01 | `PARSE_ERROR` | Malformed message |
| 0x02 | `INVALID_MESSAGE` | Failed validation |
| 0x03 | `UNKNOWN_REQUEST` | Unsupported request type |
| 0x04 | `RESOURCE_LIMIT` | Rate limited |
| 0x05 | `INTERNAL_ERROR` | Node error |
| 0x06 | `TIMEOUT` | Request timed out |

### Disconnect Reasons

| Code | Reason |
|------|--------|
| 0x01 | Requested by peer |
| 0x02 | Too many connections |
| 0x03 | Low score |
| 0x04 | Protocol violation |
| 0x05 | Incompatible version |
| 0x06 | Node shutting down |

## Version Negotiation

### Handshake

```protobuf
message Version {
    uint32 version = 1;          // Protocol version
    uint64 services = 2;         // Service flags
    uint64 timestamp = 3;        // Current time
    bytes peer_id = 4;           // Sender peer ID
    string user_agent = 5;       // Client identifier
    uint64 start_height = 6;     // Best block height
    bool relay = 7;              // Accept tx relay
}
```

### Service Flags

| Bit | Service |
|-----|---------|
| 0 | NODE_NETWORK (full node) |
| 1 | NODE_BLOOM (bloom filters) |
| 2 | NODE_WITNESS (witness data) |
| 3 | NODE_COMPACT_BLOCKS |
| 4 | NODE_QUANTUM (quantum-resistant) |
| 5 | NODE_LIGHTNING (Lightning support) |

## Quantum-Resistant Extensions

### Quantum Peer ID

Nodes may use Dilithium keys for peer identity:

```
Peer ID Format: 12D3KooW... (standard)
Quantum Flag: Set in service bits
Key Exchange: CRYSTALS-Kyber for shared secrets
Signatures: Dilithium for message signing
```

### Hybrid Mode

During transition, nodes support both classical and quantum:
- Advertise both key types
- Prefer quantum-resistant when both peers support
- Fall back to classical if necessary

---

## References

- [libp2p Specification](https://github.com/libp2p/specs)
- [Gossipsub v1.1](https://github.com/libp2p/specs/blob/master/pubsub/gossipsub/gossipsub-v1.1.md)
- [Noise Protocol Framework](https://noiseprotocol.org/noise.html)
- [CRYSTALS-Kyber](https://pq-crystals.org/kyber/)
- [CRYSTALS-Dilithium](https://pq-crystals.org/dilithium/)

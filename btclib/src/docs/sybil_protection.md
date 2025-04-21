# Sybil Attack Protection in SuperNova

This document outlines the Sybil Attack protection mechanisms implemented in the SuperNova blockchain to defend against adversaries who may attempt to create multiple identities to gain disproportionate influence over the network.

## Overview

Sybil attacks occur when a malicious actor creates multiple identities to gain influence disproportionate to the resources they control. In blockchain networks, this can lead to:

- Eclipse attacks (isolating specific nodes from the rest of the network)
- Consensus manipulation
- Transaction censorship
- Denial of service conditions

SuperNova implements several defensive mechanisms to mitigate these risks.

## Core Protection Mechanisms

### 1. Peer Diversity Management

The `PeerDiversityManager` ensures that connections are distributed across diverse network segments:

- **Subnet Diversity**: Limits connections per IP subnet (default: max 3 peers per /24 IPv4 or /48 IPv6 subnet)
- **ASN Diversity**: Limits connections per Autonomous System Number (default: max 8 peers per ASN)
- **Geographic Diversity**: Limits connections per geographic region (default: max 15 peers per region)

This prevents an attacker from dominating a node's peer list even if they control many IPs within the same network segment.

### 2. Connection Rate Limiting

Each IP address is limited in how frequently it can attempt connections:

- Default rate limit: 5 connection attempts per minute
- Automatic temporary banning for excessive connection attempts (5 minutes)
- Persistent storage of rate limit violations to prevent oscillating attacks

### 3. Peer Scoring System

Peers are scored based on multiple factors:

- **Base Score**: Increases slightly with peer age to favor long-standing network participants
- **Stability Score**: Based on connection reliability and uptime
- **Behavior Score**: Based on protocol adherence and message validity
- **Latency Score**: Based on response times
- **Diversity Score**: Higher for peers from underrepresented network segments

These scores influence peer selection for outbound connections and disconnection decisions during peer rotations.

### 4. Forced Peer Rotation

To prevent gradual isolation, SuperNova implements periodic peer rotation:

- Identifies overrepresented network segments
- Disconnects lowest-scoring peers from these segments
- Connects to high-scoring peers from underrepresented segments
- Ensures network topology remains diverse over time

## Implementation

The implementation consists of several key components:

### `PeerManager`

Central component that tracks all peers, manages connections, and enforces protection policies.

```rust
pub struct PeerManager {
    peers: HashMap<PeerId, PeerInfo>,
    connected_peers: HashSet<PeerId>,
    rate_limits: HashMap<IpAddr, RateLimitInfo>,
    diversity_manager: PeerDiversityManager,
    // Configuration parameters
    max_connection_attempts_per_min: usize,
    enable_connection_challenges: bool,
}
```

### `PeerDiversityManager`

Tracks peer distribution across network segments and calculates diversity metrics.

```rust
pub struct PeerDiversityManager {
    subnet_distribution: HashMap<IpSubnet, HashSet<PeerId>>,
    asn_distribution: HashMap<u32, HashSet<PeerId>>,
    geographic_distribution: HashMap<String, HashSet<PeerId>>,
    // Limits
    max_peers_per_subnet: usize,
    max_peers_per_asn: usize,
    max_peers_per_region: usize,
}
```

### `PeerScore`

Maintains scoring components for each peer.

```rust
pub struct PeerScore {
    base_score: f64,
    stability_score: f64,
    behavior_score: f64,
    latency_score: f64,
    diversity_score: f64,
}
```

## Usage and Configuration

Sybil protection is integrated into the P2P networking layer and is enabled by default. Key configuration options include:

- Maximum peers per network segment (subnet/ASN/region)
- Connection rate limits
- Peer rotation frequency
- Score calculation weights

These values can be adjusted in the network configuration to balance between security and connectivity requirements.

## Example: Connection Management

When a new peer connection is established:

1. The connection attempt is checked against rate limits for the source IP
2. If accepted, the peer's network segment diversity is evaluated
3. If the connection would violate diversity limits, it may be rejected
4. Otherwise, the peer is added to the connected peers list
5. The peer's score is calculated and used for future decisions

## Example: Peer Rotation

Periodically (or when triggered), the node performs peer rotation:

1. Identify subnets/ASNs/regions with too many connections
2. Select lowest-scoring peers from these segments for disconnection
3. Identify high-scoring known peers from underrepresented segments
4. Disconnect selected peers and connect to more diverse alternatives
5. Update scoring and diversity metrics

## Effectiveness and Limitations

The protection is most effective against:
- Basic Sybil attacks from limited IP ranges
- Attacks from single ASNs or geographic regions
- Connection flooding attempts

Limitations include:
- Highly distributed attackers may still gain some influence
- Requires accurate ASN and geographic data for optimal performance
- May occasionally disconnect legitimate peers during rotation

## Future Enhancements

Planned improvements include:
- Integration with reputation systems across multiple nodes
- Advanced behavioral analysis for peer evaluation
- Machine learning-based anomaly detection
- Challenge-response protocols for connection authentication
- Cryptographic proof-of-uniqueness requirements 
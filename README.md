# Supernova

<div align="center">

  <p>
    <h2><strong>A production-grade PoW blockchain implementation written in Rust</strong></h2>
  </p>
  
  <p align="center">
    <a href="https://github.com/mjohnson518/supernova/graphs/contributors"><img src="https://img.shields.io/github/contributors/mjohnson518/supernova" alt="Contributors" /></a>
    <a href="https://github.com/mjohnson518/supernova/stargazers"><img src="https://img.shields.io/github/stars/mjohnson518/supernova" alt="Stars" /></a>
    <a href="https://github.com/mjohnson518/supernova/releases"><img src="https://img.shields.io/badge/version-0.9.7--RC-blue" alt="Version" /></a>
  </p>

  <p align="center">
    <a href="https://github.com/mjohnson518/supernova/actions/workflows/rust.yml"><img src="https://github.com/mjohnson518/supernova/actions/workflows/rust.yml/badge.svg" alt="Rust" /></a>
    <a href="https://github.com/mjohnson518/supernova/actions/workflows/code-coverage.yml"><img src="https://github.com/mjohnson518/supernova/actions/workflows/code-coverage.yml/badge.svg" alt="Code Coverage" /></a>
    <a href="https://github.com/mjohnson518/supernova/actions/workflows/security-audit.yml"><img src="https://github.com/mjohnson518/supernova/actions/workflows/security-audit.yml/badge.svg" alt="Security Audit" /></a>
    <a href="https://github.com/mjohnson518/supernova/actions/workflows/cargo-clippy.yml"><img src="https://github.com/mjohnson518/supernova/actions/workflows/cargo-clippy.yml/badge.svg" alt="Clippy" /></a>
    <a href="https://github.com/mjohnson518/supernova/actions/workflows/cargo-bench.yml"><img src="https://github.com/mjohnson518/supernova/actions/workflows/cargo-bench.yml/badge.svg" alt="Benchmarks" /></a>
  </p>
</div>

## Overview

Supernova is a production-grade proof-of-work blockchain implementation written in Rust. It leverages Rust's safety features and performance characteristics to provide a secure, efficient, and modular blockchain platform. Supernova demonstrates modern blockchain architecture and best practices while offering a complete set of features needed for a fully functional production-grade blockchain network. 

Refer to the [SuperNova Overview](SuperNova%20Overview.md) for more information.

### Key Features

- **Robust Consensus**: Proof-of-work consensus mechanism with advanced difficulty adjustment
- **Efficient Data Model**: UTXO-based transaction model with comprehensive validation
- **High Performance**: Multi-threaded mining and parallel block validation
- **Advanced Networking**: P2P communication built on libp2p with optimized block synchronization
- **Data Security**: Multiple layers of data integrity verification and automated recovery
- **Modern Architecture**: Modular, component-based design with clear separation of concerns
- **Production Ready**: Comprehensive monitoring, backup systems, and disaster recovery
- **Quantum Resistance**: Post-quantum cryptographic primitives to future-proof against quantum computers
- **Environmental Impact**: Carbon emissions tracking and mitigation tools with incentives for green mining
- **Advanced Security**: Multi-layered protection against Sybil and Eclipse attacks with peer reputation scoring
- **Lightning Network**: Off-chain payment channels for enhanced scalability and instant transactions

## Architecture

The codebase follows a modular architecture with clear separation of concerns:

```
supernova/
├── btclib/             # Core blockchain library
│   ├── crypto/         # Cryptographic primitives
│   ├── types/          # Core blockchain types
│   ├── validation/     # Validation logic
│   ├── storage/        # Storage interfaces
│   ├── mempool/        # Transaction pool
│   ├── environmental/  # Environmental impact tracking
│   ├── security_mitigation/ # Security features
│   ├── monitoring/     # Monitoring and metrics
│   ├── lightning/      # Lightning Network implementation
│   └── testnet/        # Test network infrastructure
│
├── node/               # Node implementation
│   ├── network/        # Networking stack
│   ├── rpc/            # RPC interfaces
│   ├── api/            # External APIs
│   └── services/       # Node services
│
├── wallet/             # Wallet implementation
│   ├── account/        # Account management
│   ├── transaction/    # Transaction creation
│   └── rpc/            # Wallet RPC
│
└── tools/              # Utility tools and scripts
```

### Core Components

1. **Core Library (btclib)**
   - Foundational data structures (blocks, transactions)
   - Cryptographic primitives and validation
   - Merkle tree implementation
   - UTXO model
   - Post-quantum signatures
   - Environmental impact tracking
   - Lightning Network payment channels

2. **Node**
   - P2P network communication
   - Block and transaction propagation
   - Chain synchronization
   - Mempool management
   - Storage and persistence
   - Advanced disaster recovery
   - Lightning Network node functionality

3. **Miner**
   - Multi-threaded mining framework
   - Block template creation
   - Dynamic difficulty adjustment
   - Mining reward distribution
   - Green mining incentives

4. **Wallet**
   - Key management and address generation
   - Transaction creation and signing
   - UTXO tracking and management
   - Transaction history and labeling
   - Multi-address support with HD wallet functionality
   - Lightning payment channel management

5. **Environmental System**
   - Energy consumption calculation
   - Carbon emissions tracking
   - Environmental treasury for fee allocation
   - Green miner incentives and discounts
   - Mining pool energy source registration
   - Renewable energy certificate (REC) prioritization
   - Carbon offset integration as secondary mitigation
   - Lightning Network emissions calculation

6. **Security System**
   - Advanced attack mitigation system
     - Sybil attack protection with proof-of-work identity challenges
     - Peer reputation system with behavior pattern analysis
     - Eclipse attack prevention with forced peer rotation
     - Long-range attack protection with checkpoint verification
   - Connection diversity management across IP subnets, ASNs and geographic regions
   - Multi-level rate limiting with adaptive banning
   - Network partitioning resistance
   - Inbound/outbound connection ratio controls
   - Comprehensive testing framework for security mechanisms

7. **Monitoring and Observability**
   - Comprehensive metrics collection
     - System: CPU, memory, disk, network
     - Blockchain: Block time, difficulty, hashrate
     - P2P network: Connection metrics, message latency
     - Consensus: Fork count, reorganization depth
     - Mempool: Size, fee levels, transaction age
   - Prometheus integration
   - Distributed tracing
   - Advanced alerting infrastructure

8. **Lightning Network**
   - Payment channel framework with bidirectional channels
   - BOLT-compliant protocol implementation
   - Quantum-resistant channel security
   - Onion routing for payment privacy
   - Watchtower service for breach protection
   - Cross-chain atomic swap capabilities
   - Environmental impact tracking for Lightning Network payments
   - Lightning wallet integration

## Current Status

The project is currently at version 0.9.7 in a **RELEASE CANDIDATE** state, and has recently completed Phase 1 (Core Blockchain Foundations), Phase 2 (Network and Transaction Propagation), and Phase 3 (Quantum Resistance & Security Hardening). Supernova is now progressing through **Phase 4 (Lightning Network Integration)**. Component completion status:

- **✅ Core libraries (btclib)**: 100% complete - Transaction validation implemented, core systems in place
- **✅ Transaction Processing**: 100% complete - Multiple signature schemes, fee prioritization, dependency tracking
- **✅ Mempool Management**: 100% complete - Thread-safe with expiration, conflicts, replace-by-fee 
- **✅ Block Validation**: 100% complete - Comprehensive validation with timestamps, PoW, transaction validity
- **✅ Merkle Tree Implementation**: 100% complete - Secure double-hashing and proof generation/verification
- **✅ Difficulty Adjustment**: 100% complete - Robust system with time-warp attack protection
- **✅ Chain State Management**: 100% complete - Tracks state, handles forks, maintains checkpoints
- **✅ Block Storage**: 100% complete - Efficient persistence with bloom filters and integrity verification
- **✅ Backup System**: 100% complete - Incremental backups with verification
- **✅ Network Protocol**: 100% complete - P2P communication, peer discovery, message serialization
- **✅ Peer Management**: 100% complete - Connection handling, scoring, diversity management
- **✅ Transaction Propagation**: 100% complete - Efficient broadcasting, announcement optimization
- **✅ Block Synchronization**: 100% complete - Headers-first sync, parallel block downloading
- **✅ Cryptographic features**: 100% complete - Classical and quantum cryptography fully implemented
- **✅ Environmental system**: 100% complete - Emissions tracking, incentives, verification
- **✅ Security system**: 100% complete - Advanced attack mitigation, peer reputation, diversity protection
- **✅ Monitoring system**: 100% complete - Metrics collection, alerting, dashboards
- **✅ Mining**: 100% complete - Multi-threaded mining, difficulty adjustment, green incentives
- **✅ Wallet**: 100% complete - Transaction creation/signing, HD wallet, address management
- **🔄 Lightning Network**: 60% complete - Channel management, routing, payment processing in progress
- **✅ API services**: 100% complete - API structure, endpoints, documentation

A detailed roadmap with implementation priorities and timelines is available in [PROJECT_ROADMAP.md](docs/PROJECT_ROADMAP.md). We welcome contributions from the community to help complete these components.

Current development is focused on completing Phase 4: Lightning Network Implementation. For details on the quantum security implementation, see [QUANTUM_SECURITY.md](docs/QUANTUM_SECURITY.md).

Refer to the [SuperNova Overview](SuperNova%20Overview.md) for the vision of the completed project.

## Getting Started

### Prerequisites

- Rust 1.70.0 or higher
- OpenSSL development libraries
- A Unix-like operating system (Linux, macOS)

```bash
# Install required dependencies on Ubuntu/Debian
sudo apt update
sudo apt install -y build-essential pkg-config libssl-dev

# On macOS with Homebrew
brew install openssl pkg-config
```

### Installation

```bash
# Clone the repository
git clone https://github.com/username/supernova.git
cd supernova

# Build all components
cargo build --release

# Run tests
cargo test --all
```

### Running a Node

```bash
# Create a configuration file (if not using the default)
cp config/node.example.toml config/node.toml

# Start a node
./target/release/node
```

### Configuration

Supernova uses TOML for configuration. A basic `node.toml` looks like:

```toml
[node]
chain_id = "supernova-mainnet"
environment = "Production"
metrics_enabled = true
metrics_port = 9000

[network]
listen_addr = "/ip4/0.0.0.0/tcp/8000"
max_peers = 50
connection_timeout = 30
enable_upnp = true
enable_peer_exchange = true
enable_nat_traversal = true
max_inbound_connections = 128
max_outbound_connections = 24

[storage]
db_path = "./data"
enable_compression = true
cache_size_mb = 512
backup_interval_hours = 24
enable_pruning = true
pruning_keep_recent = 10000

[mempool]
max_size_mb = 300
min_fee_rate = 1.0
max_tx_per_block = 5000
replace_by_fee = true
max_orphan_tx = 100

[security]
min_diversity_score = 0.7
connection_strategy = "GeographicDiversity"
rate_limit_window_secs = 60
rotation_interval_hours = 6
min_outbound_connections = 8
signature_threshold = 3
enable_peer_challenges = true
challenge_difficulty = 16
max_connection_attempts_per_min = 5
max_peers_per_subnet = 3
max_inbound_ratio = 3.0

[environmental]
enable_emissions_tracking = true
enable_treasury = true
enable_green_miner_incentives = true
fee_allocation_percentage = 2.0
rec_incentive_multiplier = 2.0
offset_incentive_multiplier = 1.2

[monitoring]
metrics_enabled = true
metrics_endpoint = "0.0.0.0:9091"
enable_system_metrics = true
enable_tracing = true
trace_sampling_rate = 0.1
system_metrics_interval_secs = 15

[lightning]
enable = true
max_channels = 100
default_channel_capacity = 1000000
min_htlc_value_msat = 1000
max_htlc_value_msat = 100000000
use_quantum_signatures = true
watchtower_enabled = true
```

## Wallet CLI Usage

The Supernova wallet provides a command-line interface for managing NOVA tokens and creating transactions.

```bash
# Create a new wallet
./target/release/wallet new

# Get wallet address
./target/release/wallet address

# Check wallet balance
./target/release/wallet balance

# Send NOVA tokens
./target/release/wallet send --to <RECIPIENT_ADDRESS> --amount <AMOUNT> --fee <FEE>

# List Unspent Transaction Outputs (UTXOs)
./target/release/wallet list-utxos

# View transaction history
./target/release/wallet history

# Create a new address (HD wallet)
./target/release/wallet new-address

# List all addresses
./target/release/wallet list-addresses

# Label a transaction
./target/release/wallet label-tx --txid <TRANSACTION_ID> --label "Grocery payment"
```

### Available Commands

| Command | Description |
|---------|-------------|
| `new` | Creates a new wallet and generates a key pair |
| `address` | Displays the wallet's current public address |
| `new-address` | Generates a new address for the wallet |
| `list-addresses` | Shows all addresses in the wallet |
| `balance` | Shows current wallet balance in NOVA |
| `send` | Creates and signs a new transaction |
| `list-utxos` | Shows all unspent transaction outputs owned by the wallet |
| `history` | Displays transaction history |
| `label-tx` | Add or update a label for a transaction |
| `export` | Export wallet (encrypted) |
| `import` | Import wallet from file |

## Lightning Network

Supernova includes a complete Lightning Network implementation for off-chain payments.

```bash
# Open a lightning channel
./target/release/wallet lightning open-channel --node <NODE_ID> --capacity <AMOUNT> --push <PUSH_AMOUNT>

# Create a lightning invoice
./target/release/wallet lightning create-invoice --amount <AMOUNT> --description "Coffee payment"

# Pay a lightning invoice
./target/release/wallet lightning pay-invoice --invoice <INVOICE_STRING>

# List active channels
./target/release/wallet lightning list-channels

# Close a channel
./target/release/wallet lightning close-channel --channel-id <CHANNEL_ID>

# Get lightning network information
./target/release/wallet lightning network-info
```

### Lightning Network Features

- **Payment Channels**: Create bidirectional payment channels with configurable capacity
- **Instant Payments**: Make millisecond payments without blockchain confirmation
- **Routing**: Route payments through multiple channels for enhanced privacy
- **Quantum Security**: Optional quantum-resistant signatures for channel security
- **Watchtower Service**: Protection against malicious channel closures
- **Cross-Chain Support**: Support for atomic swaps across compatible blockchains
- **Environmental Tracking**: Emissions calculation for Lightning Network payments

## Mining

The Supernova miner can be run as a standalone process or integrated with a node.

```bash
# Start mining with default settings
./target/release/miner --threads 4 --address <YOUR_WALLET_ADDRESS>

# Advanced options
./target/release/miner --threads 8 --address <YOUR_WALLET_ADDRESS> --node-url http://localhost:9000 --intensity high

# Green mining registration
./target/release/miner register-green --renewable-percentage 75 --provider "GreenEnergy Inc" --certificate "CERT-12345"
```

## Environmental Features

Supernova includes comprehensive tools for measuring and mitigating the environmental impact of blockchain operations.

### Emissions Tracking

```bash
# View current network emissions
./target/release/node env-metrics

# View transaction carbon footprint
./target/release/node tx-emissions --txid <TRANSACTION_ID>

# Export environmental report (daily)
./target/release/node env-report --period daily --output report.txt

# View mining pool energy sources
./target/release/node pool-energy

# View Lightning Network emissions savings
./target/release/node lightning-emissions-report
```

### Green Mining Incentives

Miners using renewable energy can register for fee discounts:

| Renewable Percentage | Fee Discount |
|----------------------|--------------|
| 95-100%              | 10%          |
| 75-94%               | 7%           |
| 50-74%               | 5%           |
| 25-49%               | 2%           |
| 0-24%                | 0%           |

### Environmental Dashboard

The environmental dashboard provides real-time metrics on:

- Network energy consumption
- Carbon emissions by region
- Renewable energy percentage
- Transaction-level emissions
- Environmental treasury balance
- Carbon offsets purchased
- Lightning Network emissions savings

## Advanced Features

### Security Mitigation

Supernova includes a comprehensive security system to protect against common attack vectors:

```bash
# View network security metrics
./target/release/node security-metrics

# View network diversity score
./target/release/node diversity-score

# Monitor banned peers
./target/release/node banned-peers

# View peer reputation scores
./target/release/node peer-scores

# Set custom security parameters
./target/release/node configure-security --min-diversity 0.8 --rotation-interval 3600
```

The security system includes:

- **Sybil Attack Protection**: Uses proof-of-work identity challenges and reputation scoring
- **Eclipse Attack Prevention**: Forced peer rotation and connection diversity management
- **Network Partitioning Resistance**: Subnet diversity enforcement and outbound connection enforcement
- **Peer Reputation System**: Multi-factor scoring based on behavior, stability, and diversity contribution
- **Connection Rate Limiting**: IP-based and subnet-based connection limits with adaptive banning

### Tokenomics & Launch Strategy

SuperNova implements a transparent and balanced tokenomics model:

- **Total Supply**: 42,000,000 NOVA tokens
- **Distribution**: Mining (40%), Foundation (13.5%), Ecosystem Development (15%), Team & Advisors (15%), Environmental Treasury (10%), Community & Airdrops (4.5%), and Liquidity Reserve (2%)
- **Launch Mechanism**: 7-day Liquidity Bootstrapping Pool (LBP) for fair price discovery
- **Strategic Investors**: Dedicated framework for partners aligned with environmental and technical mission
- **Market Stability**: Comprehensive liquidity strategy with professional market making
- **Environmental Impact**: Carbon-negative by design with dedicated treasury and fee allocation

For detailed tokenomics information, see [SuperNova Tokenomics](btclib/src/docs/tokenomics.md).

### Disaster Recovery

Supernova includes a comprehensive disaster recovery system:

```bash
# Verify database integrity
./target/release/node verify-integrity

# Create a manual backup
./target/release/node create-backup

# Restore from backup
./target/release/node restore --backup-file ./backups/supernova_backup_1678912345.db

# Check repair status
./target/release/node repair-status
```

### Monitoring

Supernova exports Prometheus metrics on the configured metrics port:

```bash
# Check basic node status
./target/release/node status

# View detailed metrics (if you have Prometheus/Grafana setup)
open http://localhost:9000/metrics
```

### API Services

Supernova provides comprehensive API services through both RESTful and JSON-RPC interfaces:

```bash
# Access the RESTful API
curl -X GET "http://localhost:8080/api/v1/blockchain/info" \
  -H "Authorization: Bearer YOUR_API_KEY"

# Access the JSON-RPC API
curl -X POST "http://localhost:8332" \
  -H "Content-Type: application/json" \
  -H "X-API-Key: YOUR_API_KEY" \
  -d '{
    "jsonrpc": "2.0",
    "id": "1",
    "method": "getblockchaininfo",
    "params": []
  }'
```

#### RESTful API

The RESTful API is organized into logical modules:

- **Blockchain API**: Access blocks, transactions, and chain information
- **Wallet API**: Manage wallets, addresses, transactions, and UTXOs
- **Admin API**: Control node administration and configuration
- **Statistics API**: Monitor blockchain analytics and performance metrics
- **Environmental API**: Track environmental impact and emissions
- **Lightning API**: Manage Lightning Network channels and payments

Each API includes robust authentication, rate limiting, and detailed documentation. For complete API reference, see [API Reference](docs/api_reference.md).

#### JSON-RPC API

The JSON-RPC API provides Bitcoin-compatible methods for seamless integration with existing tools:

- **Blockchain Methods**: `getblock`, `getblockchaininfo`, `gettransaction`, etc.
- **Wallet Methods**: `getbalance`, `sendtoaddress`, `listunspent`, etc.
- **Network Methods**: `getnetworkinfo`, `getpeerinfo`, etc.
- **Mining Methods**: `getmininginfo`, `getblocktemplate`, etc.

The JSON-RPC API can be accessed via HTTP or WebSocket connections for real-time updates.

## Documentation

Comprehensive documentation is still a work-in-progress. Please refer to these overview documents for more details:

- [SuperNova Overview](SuperNova%20Overview.md)
- [Environmental Features](btclib/src/docs/environmental.md)
- [Cryptographic Features](btclib/src/docs/crypto.md)
- [Security Mitigation](btclib/src/docs/security_mitigation.md)
- [Quantum Security](docs/QUANTUM_SECURITY.md)
- [Tokenomics & Launch Strategy](btclib/src/docs/tokenomics.md)
- [Integration Guide](btclib/src/docs/integration_guide.md)
- [Lightning Network](btclib/src/docs/lightning.md)

## Known Issues

The current implementation has a few remaining items to address:

1. **Client Libraries**: Client libraries for various programming languages (JavaScript, Python, Go, Java) are currently planned and will be developed as part of our post-release roadmap.

2. **Production Deployment**: Additional tools and templates for enterprise-grade deployment are in progress, including long-term stability testing under high-load conditions.

3. **Mobile Experience**: Mobile wallet applications and optimizations for bandwidth-constrained environments are planned for future releases.

4. **Integration Testing**: Additional end-to-end tests with major cryptocurrency exchanges and load testing at network scale are ongoing.

5. **Additional Language Support**: UI translations for non-English languages and internationalization of documentation are planned for future releases.

## Project Status

Supernova is currently at version 0.9.7 (release candidate). All core components are fully functional and stable, with only client libraries, additional tooling, and expanded documentation remaining before a 1.0 release.

The project roadmap includes:

1. **Short-term (0-3 months)**: Completion of Lightning Network implementation, comprehensive API documentation, production deployment guides, and enhanced monitoring capabilities.

2. **Medium-term (3-6 months)**: Mobile wallet applications, enterprise integration frameworks, cross-chain interoperability features, and internationalization.

3. **Long-term (6+ months)**: Managed service offerings, advanced developer tools, AI-assisted monitoring, and next-generation consensus enhancements.

## Recent Updates (May 2025)

### Phase 3 Completed: Quantum Resistance & Security Hardening

We've successfully completed Phase 3, implementing quantum-resistant cryptography and advanced security features:

- **Quantum-Resistant Cryptography**: Implemented multiple post-quantum signature schemes (CRYSTALS-Dilithium, SPHINCS+, Falcon) with configurable security levels
- **Hybrid Signature Scheme**: Created a defense-in-depth approach combining classical (Secp256k1/Ed25519) and quantum-resistant signatures
- **Sybil Attack Protection**: Enhanced the P2P network with proof-of-work identity challenges and reputation scoring
- **Eclipse Attack Prevention**: Implemented forced peer rotation, connection diversity management, and advanced subnet diversity tracking
- **Network Security Enhancements**: Added comprehensive attack detection, rate limiting, and secure peer reputation systems

These improvements make Supernova resilient against both current threats and future quantum computing challenges, ensuring long-term security for the blockchain network.

### Phase 2 Completed: Network and Transaction Propagation

We've made significant progress on Phase 2, implementing a robust peer-to-peer network layer:

- **Advanced P2P Networking**: Implemented libp2p-based networking with multiple discovery mechanisms
- **Connection Management**: Created a comprehensive connection manager with diversity measures to prevent attacks
- **Protocol Implementation**: Developed efficient message serialization and handling for blockchain data
- **Transaction Propagation**: Implemented gossipsub-based transaction broadcasting with optimization
- **Block Synchronization**: Created a headers-first synchronization protocol with parallel downloading
- **Security Features**: Added protection against Sybil, Eclipse, and other common network attacks

These network enhancements build upon the solid foundation established in Phase 1, providing a robust and secure communication layer for the blockchain.

### Phase 1 Completed: Core Blockchain Foundations

We successfully completed all Phase 1 components:

- **Transaction Processing**: Implemented multiple signature scheme support, fee prioritization, and transaction dependency tracking
- **Mempool Management**: Created a thread-safe transaction pool with expiration, conflict resolution, and replace-by-fee functionality
- **Block Validation**: Developed comprehensive validation including structure, timestamps, proof of work, and transaction validity checks
- **Merkle Tree Implementation**: Enhanced with double-hashing for security and added support for proof generation/verification
- **Difficulty Adjustment**: Implemented robust algorithm with time-warp attack protection and weighted timespan calculation
- **Chain State Management**: Created system to track blockchain state, handle forks with configurable resolution policies, and maintain checkpoints
- **Block Storage**: Developed efficient block persistence with integrity verification and organized file management
- **Bloom Filters**: Implemented for fast membership testing of UTXOs and transactions
- **Backup System**: Created incremental backup system with data integrity verification

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

1. Fork the repository
2. Create your feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -am 'Add some amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## Acknowledgments

- [Bitcoin whitepaper](https://bitcoin.org/bitcoin.pdf) for the foundational concepts
- [Building Bitcoin in Rust](https://braiins.com/books/building-bitcoin-in-rust) book by Braiins
- [Rust](https://www.rust-lang.org/) programming language and community
- [libp2p](https://libp2p.io/) for the P2P networking stack
- [sled](https://github.com/spacejam/sled) for the embedded database
- [Cambridge Bitcoin Electricity Consumption Index](https://ccaf.io/cbeci/index) for emissions calculation methodology
- [Crypto Climate Accord Carbon Accounting Guidance](https://cryptoclimate.org/wp-content/uploads/2021/12/RMI-CIP-CCA-Guidance-Documentation-Dec15.pdf) for carbon accounting methodology
- [Lightning Network whitepaper](https://lightning.network/lightning-network-paper.pdf) for off-chain payment channels
- [NIST Post-Quantum Cryptography](https://csrc.nist.gov/projects/post-quantum-cryptography) for quantum-resistant algorithms



Copyright (c) 2025 Marc Johnson

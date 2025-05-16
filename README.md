# Supernova

<div align="center">

  <p>
    <h2><strong>A production-grade PoW blockchain implementation written in Rust</strong></h2>
  </p>
  
  <p align="center">
    <a href="https://supernovanetwork.xyz/"><img src="https://img.shields.io/badge/website-supernovanetwork.xyz-blue" alt="Official Website" /></a>
    <a href="https://github.com/mjohnson518/supernova/graphs/contributors"><img src="https://img.shields.io/github/contributors/mjohnson518/supernova" alt="Contributors" /></a>
    <a href="https://github.com/mjohnson518/supernova/stargazers"><img src="https://img.shields.io/github/stars/mjohnson518/supernova" alt="Stars" /></a>
    <a href="https://github.com/mjohnson518/supernova/releases"><img src="https://img.shields.io/badge/version-0.9.9--RC-blue" alt="Version" /></a>
     <a href="https://deepwiki.com/mjohnson518/supernova"><img src="https://deepwiki.com/badge.svg" alt="Ask DeepWiki"></a>
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

The project is currently at version 0.9.9 in a **FINAL RELEASE CANDIDATE** state, and has successfully completed Phase 1 (Core Blockchain Foundations), Phase 2 (Network and Transaction Propagation), Phase 3 (Quantum Resistance & Security Hardening), Phase 4 (Environmental Features), Phase 5 (Lightning Network Integration), and Phase 6 (Production Readiness). Supernova is now ready for production deployment with full optimization, monitoring, scaling, and disaster recovery capabilities.

Component completion status:

- **✅ Core libraries (btclib)**: 100% complete 
- **✅ Transaction Processing**: 100% complete
- **✅ Mempool Management**: 100% complete
- **✅ Block Validation**: 100% complete
- **✅ Merkle Tree Implementation**: 100% complete
- **✅ Difficulty Adjustment**: 100% complete
- **✅ Chain State Management**: 100% complete
- **✅ Block Storage**: 100% complete
- **✅ Backup System**: 100% complete
- **✅ Network Protocol**: 100% complete
- **✅ Peer Management**: 100% complete
- **✅ Transaction Propagation**: 100% complete
- **✅ Block Synchronization**: 100% complete
- **✅ Cryptographic features**: 100% complete
- **✅ Environmental system**: 100% complete
- **✅ Security system**: 100% complete
- **✅ Monitoring system**: 100% complete
- **✅ Mining**: 100% complete
- **✅ Wallet**: 100% complete
- **✅ Lightning Network**: 100% complete
- **✅ API services**: 100% complete
- **✅ Optimization & Performance**: 100% complete
- **✅ Deployment & Infrastructure**: 100% complete

A detailed roadmap with implementation priorities and timelines is available in [PROJECT_ROADMAP.md](PROJECT_ROADMAP.md). We welcome contributions from the community to help complete these components.

Current development is focused on preparing for the 1.0.0 final release, which will incorporate user feedback and polish all existing functionality.

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
- **Distribution**: Mining (40%), Foundation (13.5%), Ecosystem Development (15%), Team & Advisors (10%), Environmental Treasury (10%), Community & Airdrops (7.5%), and Liquidity Reserve (4%)
- **Launch Mechanism**: 7-day Liquidity Bootstrapping Pool (LBP) for fair price discovery
- **Strategic Investors**: Dedicated framework for partners aligned with environmental and technical mission
- **Market Stability**: Comprehensive liquidity strategy with professional market making
- **Environmental Impact**: Carbon-negative by design with dedicated treasury and fee allocation
- **Exchange Strategy**: Phased approach with mid-tier exchanges first, followed by tier-1 platforms

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

- **Blockchain Methods**: `getblock`, `
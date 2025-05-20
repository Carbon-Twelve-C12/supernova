# Supernova

<div align="center">

  <p>
    <h2><strong>A production-grade PoW blockchain implementation written in Rust</strong></h2>
  </p>
  
  <p align="center">
    <a href="https://supernovanetwork.xyz/"><img src="https://img.shields.io/badge/website-supernovanetwork.xyz-blue" alt="Official Website" /></a>
    <a href="https://github.com/mjohnson518/supernova/graphs/contributors"><img src="https://img.shields.io/github/contributors/mjohnson518/supernova" alt="Contributors" /></a>
    <a href="https://github.com/mjohnson518/supernova/stargazers"><img src="https://img.shields.io/github/stars/mjohnson518/supernova" alt="Stars" /></a>
    <a href="https://github.com/mjohnson518/supernova/releases"><img src="https://img.shields.io/badge/version-0.7.0--DEV-blue" alt="Version" /></a>
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

The project is currently at version 0.7.0 in **ACTIVE DEVELOPMENT** state. While the architecture and core components have been designed, many features are still in development or have partial implementations. We've recently made significant progress in resolving compilation issues and improving code quality.

Key points:

- **Compilation Progress**: Major compilation errors have been fixed, particularly in validation systems, cryptographic verification, and environmental tracking. Some isolated issues remain in the testnet network simulation.
- **Quantum Resistance**: Post-quantum cryptographic signatures module is now fully integrated and compatible with the validation framework, supporting Dilithium, Falcon, and SPHINCS+ schemes.
- **Environmental Features**: Fixed compatibility issues in the environmental API and treasury system, enabling accurate tracking of energy usage and carbon offsets.
- **Transaction Validation**: Enhanced validation framework with comprehensive error handling, including support for both classical and post-quantum signature schemes.
- **Error Handling**: Implemented robust error handling throughout the codebase, particularly in the validation and cryptographic modules.

### Implementation Status

Component statuses:

- **Core libraries (btclib)**: ~80% complete (key structures implemented, remaining issues being addressed)
- **Transaction Processing**: ~75% complete (validation framework fully implemented)
- **Mempool Management**: ~60% complete (basic structure with partial functionality)
- **Transaction Validation**: ~95% complete (comprehensive validation with quantum signature support)
- **Block Validation**: ~65% complete (basic validation with some advanced features missing)
- **Merkle Tree Implementation**: 100% complete
- **Network Layer**: ~40% complete (peer discovery and sync partially implemented)
- **Storage**: ~75% complete (disk storage for blocks and UTXO set mostly complete)
- **Consensus Engine**: ~60% complete (proof-of-work implementation with difficulty adjustment)
- **RPC API**: ~30% complete (basic node control and query endpoints)
- **Environmental Monitoring**: ~90% complete (tracking system for energy usage and carbon footprint)
- **Wallet**: ~25% complete (basic functionality only)
- **CLI**: ~35% complete

### Known Issues

- **NetworkSimulationConfig**: Type conflicts between different module imports of the same type
- **Deprecation Warnings**: Some deprecated method calls in DateTime handling need updating
- **Unused Variables**: Cleanup needed for unused variables and imports
- **Test Coverage**: Needs expansion, particularly for recent fixes and enhancements

### Development Roadmap

1. **Current Phase (Finishing)**: Core system implementation and compilation issue fixes
2. **Next Phase (Mid Q3 2023)**: Testnet environment and improved testing infrastructure
3. **Future Phase (Q4 2023)**: Enhanced security, quantum resistance, and environmental features

For detailed information about the current development status and next steps, see [DEVELOPMENT_STATUS.md](DEVELOPMENT_STATUS.md) and [SuperNova Overview.md](SuperNova%20Overview.md).

## Getting Started

⚠️ **IMPORTANT**: This project is in active development with some remaining compilation errors. It is not yet suitable for production use.

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

## Testnet Environment

A Docker-based testnet environment is available for development and testing:

### Running the Testnet

```bash
# Start the testnet
./run_testnet.sh start

# Check status
./run_testnet.sh status

# View logs
./run_testnet.sh logs

# Stop the testnet
./run_testnet.sh stop
```

### Interacting with the Testnet

The testnet comes with a CLI client for interaction:

```bash
# Run the CLI client
cargo run --package supernova-cli

# Use specific commands
cargo run --package supernova-cli -- status
cargo run --package supernova-cli -- balance 0x123456789abcdef
```

Note: The current testnet implementation uses simulated nodes rather than actual blockchain nodes. It provides a realistic environment for basic testing but doesn't fully implement all blockchain features.

## Development Roadmap

Our current development priorities are:

1. **Phase 1 (In Progress)**: Core Blockchain Functionality
   - Complete implementation of core data structures
   - Implement fully functional transaction validation
   - Develop robust chain state management
   - Fix remaining compilation issues

2. **Phase 2 (Planned)**: Networking and Consensus
   - Implement P2P networking with libp2p
   - Develop block synchronization protocol
   - Create peer discovery and management
   - Implement fork resolution

3. **Phase 3 (Planned)**: Advanced Features
   - Implement quantum-resistant cryptography
   - Develop environmental impact tracking
   - Create security hardening mechanisms
   - Implement Lightning Network functionality

4. **Phase 4 (Planned)**: Production Readiness
   - Develop monitoring and observability
   - Implement disaster recovery
   - Create deployment tools
   - Optimize performance

## License

SuperNova is licensed under MIT License.
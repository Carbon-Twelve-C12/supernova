# supernova

SuperNova is a proof-of-work blockchain implementation written in Rust, designed to demonstrate modern blockchain architecture while leveraging Rust's safety features and performance characteristics. The system will allow for creating and validating transactions, mining new blocks, and maintaining a secure, decentralized ledger across multiple nodes. This document provides technical specifications and implementation guidelines for the project.

## Features

- Proof-of-work consensus mechanism
- UTXO-based transaction model
- Multi-threaded mining
- P2P networking with libp2p
- CLI wallet interface

## Components

### Core Library (btclib)
- Block and transaction structures
- Merkle tree implementation
- Cryptographic primitives
- UTXO model implementation

### Node
- P2P network communication
- Block and transaction propagation
- Chain synchronization
- Mempool management
- Storage and persistence

### Miner
- Multi-threaded mining
- Block template creation
- Dynamic difficulty adjustment
- Mining reward management

### Wallet CLI
The SuperNova wallet provides a command-line interface for managing NOVA tokens and creating transactions.

#### Installation
```bash
cargo build
```

#### Usage
The wallet binary provides several commands for managing your NOVA tokens:

```bash
# Create a new wallet
cargo run -- new

# Get wallet address
cargo run -- address

# Check wallet balance
cargo run -- balance

# Send NOVA tokens
cargo run -- send --to <RECIPIENT_ADDRESS> --amount <AMOUNT> --fee <FEE>

# List Unspent Transaction Outputs (UTXOs)
cargo run -- list-utxos
Optional Parameters

--wallet <PATH>: Specify custom wallet file path (default: wallet.json)
```

### Commands

| Command | Description |
|---------|-------------|
| `new` | Creates a new wallet and generates a key pair |
| `address` | Displays the wallet's public address |
| `balance` | Shows current wallet balance in NOVA |
| `send` | Creates and signs a new transaction |
| `list-utxos` | Shows all unspent transaction outputs owned by the wallet |

#### Send Command Options
| Option | Description | Required |
|--------|-------------|----------|
| `--to` | Recipient's address | Yes |
| `--amount` | Amount of NOVA to send | Yes |
| `--fee` | Transaction fee in NOVA | No (default: 1) |

### Examples

```bash
#Create a new wallet
cargo run -- new

# Send 100 NOVA with a 1 NOVA fee
cargo run -- send --to 0123456789abcdef... --amount 100 --fee 1

# Use a custom wallet file
cargo run -- --wallet my_wallet.json balance
Building
bashCopy# Build all components
cargo build

# Run tests
cargo test
```

### Technical Details

Written in Rust
Uses libp2p for P2P networking
Uses sled for database storage
Uses secp256k1 for cryptographic operations
Implements SHA-256 for proof-of-work

### License

MIT License

Copyright (c) 2024 Marc Johnson
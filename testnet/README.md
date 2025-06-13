# supernova Testnet

This module provides the testnet environment runner for the Supernova blockchain.

## Overview

The testnet runner provides a development environment for testing supernova features, including:

- Quantum-resistant cryptography
- Environmental sustainability tracking
- Transaction validation
- Network operations

## Usage

### Starting the Testnet

To start the testnet:

```bash
cargo run --package supernova-testnet
```

Or with custom options:

```bash
cargo run --package supernova-testnet -- --nodes=5
```

### Docker Setup

The testnet can also be run using Docker:

```bash
./scripts/run_testnet.sh start
```

### CLI Interaction

Use the CLI to interact with the testnet:

```bash
cargo run --package supernova-cli
```

## Configuration

The testnet uses default configurations suitable for local development. You can modify these settings in the following files:

- `docker-compose.yml` - Container configuration
- `config/testnet.toml` - Network parameters

## Project Structure

- `src/main.rs` - Main entry point for the testnet launcher
- `docker/` - Docker configuration files
- `config/` - Configuration files for testnet nodes

## Development

When developing with the testnet, you can:

1. Modify node configurations in the `config/` directory
2. Update Docker settings in `docker-compose.yml`
3. Implement custom node behavior by modifying the testnet binary

For more information, see the [testnet setup guide](../TESTNET_SETUP.md). 
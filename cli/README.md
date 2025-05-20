# SuperNova CLI

This module provides a command-line interface for interacting with the SuperNova blockchain.

## Overview

The SuperNova CLI offers a simple way to interact with the blockchain, supporting operations like:

- Checking node and network status
- Managing wallets and addresses
- Sending transactions
- Mining blocks
- Viewing environmental metrics

## Usage

### Starting the CLI

To start the CLI:

```bash
cargo run --package supernova-cli
```

This will start an interactive shell. You can also run specific commands directly:

```bash
cargo run --package supernova-cli -- status
cargo run --package supernova-cli -- balance 0x123456789abcdef
```

### Interactive Mode

In interactive mode, you can enter commands at the prompt:

```
> status
Network: Testnet
Status: Running
Nodes: 3 active
Block height: 1000
Difficulty: 12345

> balance 0x123456789abcdef
Address: 0x123456789abcdef
Balance: 100.0 NOVA
```

## Available Commands

The CLI supports the following commands:

- `help` - Show help information
- `version` - Show version information
- `status` - Show network status
- `balance [ADDRESS]` - Show balance for an address
- `send [TO] [AMOUNT]` - Send NOVA to an address
- `mine [THREADS]` - Start mining

## Project Structure

- `src/main.rs` - Main entry point and command handler
- `src/commands/` - Implementation of individual commands
- `src/utils/` - Utility functions for common operations

## Development

When developing the CLI, you can:

1. Add new commands by adding command handlers in the main file
2. Implement more sophisticated output formatting
3. Add more advanced wallet management features

The CLI is designed to be extended with more capabilities as needed. 
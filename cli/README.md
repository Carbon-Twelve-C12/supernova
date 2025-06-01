# Supernova CLI

Command-line interface for interacting with the Supernova blockchain network.

## Features

- **Blockchain Operations**: Check status, view peers, monitor mempool, track environmental metrics
- **Wallet Management**: Create/import wallets with Supernova addresses, manage addresses, check balances
- **Transactions**: Send NOVA, track transactions, view history
- **Mining**: Start/stop mining, run benchmarks, monitor mining status
- **Configuration**: Manage settings, switch networks, customize output

## Supernova Address Format

Supernova uses its own address format with network-specific prefixes:
- **Mainnet**: Addresses start with `SN` (e.g., `SN1A2B3C4D...`)
- **Testnet**: Addresses start with `ST` (e.g., `ST1A2B3C4D...`)
- **Devnet**: Addresses start with `SD` (e.g., `SD1A2B3C4D...`)

Addresses are derived using BIP44 HD wallet standards and encoded using Base58Check encoding.

## Installation

### From Source
```bash
cargo install --path .
```

### Pre-built Binary
Download the latest release from the [releases page](https://github.com/mjohnson518/supernova).

## Quick Start

1. **Check blockchain status**:
   ```bash
   supernova status
   ```

2. **Create a wallet**:
   ```bash
   supernova wallet create
   ```

3. **Check balance**:
   ```bash
   supernova balance
   ```

4. **Send NOVA**:
   ```bash
   supernova send <address> <amount>
   ```

## Commands

### Blockchain Commands

```bash
# Show blockchain status
supernova blockchain status

# List connected peers
supernova blockchain peers

# Show mempool information
supernova blockchain mempool

# Show environmental metrics
supernova blockchain environmental
```

### Wallet Commands

```bash
# Create new wallet (generates Supernova addresses)
supernova wallet create [name]

# Import wallet from mnemonic
supernova wallet import [name]

# List all wallets
supernova wallet list

# Check balance (using Supernova address)
supernova wallet balance [address]

# Generate new Supernova address
supernova wallet new-address [wallet]

# Export private keys (dangerous!)
supernova wallet export <wallet>
```

### Transaction Commands

```bash
# Send NOVA
supernova transaction send <to> <amount>

# Get transaction details
supernova transaction get <txid>

# Show transaction history
supernova transaction history [address]
```

### Mining Commands

```bash
# Show mining status
supernova mining status

# Start mining
supernova mining start [--threads <num>]

# Stop mining
supernova mining stop

# Run benchmark
supernova mining benchmark
```

### Configuration Commands

```bash
# Show configuration
supernova config show

# Set configuration value
supernova config set <key> <value>

# Reset to defaults
supernova config reset

# Interactive configuration
supernova config interactive
```

## Configuration

The CLI stores configuration in `~/.supernova/cli/config.toml`.

### Configuration Options

- `rpc_url`: RPC endpoint URL (default: `http://localhost:9332`)
- `network`: Network to connect to (`mainnet`, `testnet`, `devnet`)
- `timeout`: Request timeout in seconds (default: 30)
- `debug`: Enable debug logging (default: false)
- `output_format`: Output format (`table`, `json`, `text`)

### Environment Variables

- `SUPERNOVA_RPC_URL`: Override RPC URL
- `SUPERNOVA_NETWORK`: Override network

### Command-line Options

- `--rpc-url <url>`: Override RPC URL for this command
- `--network <network>`: Override network for this command
- `-f, --format <format>`: Output format (json/table/text)
- `-d, --debug`: Enable debug output

## Examples

### Basic Usage

```bash
# Check status with JSON output
supernova status -f json

# Create testnet wallet (generates ST... addresses)
supernova wallet create my-wallet --network testnet

# Send 10 NOVA to a Supernova address
supernova send ST1A2B3C4D5E6F7G8H9I0J 10.0

# Start mining with 4 threads
supernova mining start --threads 4
```

### Advanced Usage

```bash
# Connect to custom RPC endpoint
supernova --rpc-url http://192.168.1.100:9332 blockchain status

# Export wallet configuration
supernova config show -f json > config.json

# Batch operations
for addr in $(supernova wallet list -f json | jq -r '.[]'); do
  supernova balance $addr
done
```

## Security

- **Private Keys**: Never share your private keys or mnemonic phrases
- **Wallet Files**: Stored locally in `~/.supernova/wallets/`
- **Address Format**: Only accept addresses with proper Supernova prefixes (SN/ST/SD)
- **RPC Security**: Use HTTPS and authentication for remote nodes
- **Export Warning**: The `wallet export` command shows private keys in hex format - use with caution

## Development

### Building from Source

```bash
# Clone repository
git clone https://github.com/mjohnson518/supernova
cd supernova/cli

# Build release version
cargo build --release

# Run tests
cargo test

# Install locally
cargo install --path .
```

### Running Tests

```bash
# Unit tests
cargo test

# Integration tests
cargo test --test '*' -- --test-threads=1

# With debug output
RUST_LOG=debug cargo test
```

## Troubleshooting

### Connection Issues

```bash
# Check if node is running
supernova status

# Test with different RPC URL
supernova --rpc-url http://localhost:9332 status

# Enable debug logging
supernova -d status
```

### Wallet Issues

```bash
# List wallet files
ls ~/.supernova/wallets/

# Check wallet info
supernova wallet list -f json
```

### Mining Issues

```bash
# Check system resources
supernova mining benchmark

# Start with fewer threads
supernova mining start --threads 1
```

## License

MIT License - see LICENSE file for details. 
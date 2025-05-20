# SuperNova Testnet Setup Guide

This document provides detailed instructions for setting up and running the SuperNova blockchain testnet.

## Prerequisites

Before you can run the SuperNova testnet, make sure you have the following prerequisites installed:

- Docker
- Docker Compose
- Git
- Rust and Cargo (for building from source)

## Installation

### Step 1: Clone the Repository

```bash
git clone https://github.com/yourusername/supernova.git
cd supernova
```

### Step 2: Configuration

The testnet is pre-configured with sensible defaults. However, you can customize the following:

1. Number of nodes: Edit `docker-compose.yml` to add or remove node services
2. Network parameters: Edit `config/testnet.toml` to adjust block time, difficulty, etc.
3. Resource allocation: Edit `docker-compose.yml` to adjust memory and CPU limits

### Step 3: Start the Testnet

Use the provided script to start the testnet:

```bash
./run_testnet.sh start
```

Alternatively, you can use Docker Compose directly:

```bash
docker-compose up -d
```

### Step 4: Verify the Setup

Check that all services are running:

```bash
./run_testnet.sh status
```

You should see all nodes and the monitoring service in a "running" state.

## Using the Testnet

### Viewing Logs

To view the logs from all nodes:

```bash
./run_testnet.sh logs
```

Or for a specific node:

```bash
docker logs -f supernova-node1
```

### Interacting with the Blockchain

#### Using the CLI Client

The SuperNova CLI client provides a command-line interface to interact with the blockchain:

```bash
cargo run --bin cli
```

This will open an interactive shell where you can enter commands.

#### Available Commands

- `help` - Show help information
- `version` - Show version information
- `status` - Show network status
- `balance [ADDRESS]` - Show balance for an address
- `send [TO] [AMOUNT]` - Send NOVA to an address
- `mine [THREADS]` - Start mining

#### Example Usage

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

> send 0x123456789abcdef 10.5
Sending 10.5 NOVA to 0x123456789abcdef
Transaction submitted: 0x9876543210abcdef
```

### Monitoring the Network

The monitoring dashboard is available at `http://localhost:9900`. It provides the following information:

- Network status
- Block explorer
- Node status
- Transaction pool status
- Environmental metrics

### Stopping the Testnet

To stop the testnet:

```bash
./run_testnet.sh stop
```

## Advanced Usage

### Adding Custom Nodes

To add a custom node:

1. Edit `docker-compose.yml` to add a new node service
2. Update the environment variables to specify unique node ID and ports
3. Restart the testnet using `./run_testnet.sh restart`

### Testing Features

#### Quantum Resistance

The testnet allows testing of quantum-resistant cryptographic signatures:

1. Generate a quantum-resistant key pair:
   ```
   cargo run --bin cli quantum-keygen
   ```

2. Send a transaction using the quantum-resistant key:
   ```
   cargo run --bin cli send-quantum [ADDRESS] [AMOUNT] --key-file=quantum_key.json
   ```

#### Environmental Monitoring

The environmental dashboard shows the carbon intensity of the network:

1. Access the monitoring dashboard at `http://localhost:9900`
2. Click on "Environmental Metrics" to view carbon intensity data
3. Filter by time period or region

## Troubleshooting

### Common Issues

1. **Nodes not connecting**: Check firewall settings and ensure Docker network is properly configured

2. **Slow performance**: Adjust resource allocation in `docker-compose.yml`

3. **Port conflicts**: Change the port mappings in `docker-compose.yml` if you have conflicts

### Getting Help

If you encounter any issues, please:

1. Check the logs for error messages
2. Consult the [SuperNova Documentation](https://github.com/yourusername/supernova/wiki)
3. Submit an issue on GitHub if you believe you've found a bug 
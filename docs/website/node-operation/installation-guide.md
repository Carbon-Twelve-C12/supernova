# Supernova Node Installation Guide

## Overview

This guide provides comprehensive instructions for installing and configuring a Supernova blockchain node. Whether you're setting up a full node, mining node, or just want to participate in the network, this guide covers all necessary steps and configuration options.

## System Requirements

### Minimum Requirements

| Component | Requirement |
|-----------|-------------|
| CPU | 4 cores, 2.0 GHz+ |
| RAM | 8 GB |
| Storage | 500 GB SSD |
| Network | 10 Mbps broadband, unlimited data |
| Operating System | Ubuntu 20.04/22.04, Debian 11, macOS 12+, or Windows 10/11 |

### Recommended Specifications

| Component | Recommendation |
|-----------|----------------|
| CPU | 8+ cores, 3.0 GHz+ |
| RAM | 16+ GB |
| Storage | 1+ TB NVMe SSD |
| Network | 100+ Mbps, unlimited data |
| Operating System | Ubuntu 22.04 LTS or Debian 11 |

### Mining Node Requirements

| Component | Requirement |
|-----------|-------------|
| CPU | 12+ cores, 3.5 GHz+ |
| RAM | 32+ GB |
| Storage | 1+ TB NVMe SSD |
| Network | 100+ Mbps, unlimited data |
| GPU | Optional: CUDA-compatible for hardware acceleration |

## Prerequisites

Before installation, ensure your system meets these prerequisites:

### Ubuntu/Debian

```bash
# Update package lists
sudo apt update

# Install required dependencies
sudo apt install -y build-essential pkg-config libssl-dev curl git jq

# Optional: Install monitoring tools
sudo apt install -y htop iotop
```

### macOS

```bash
# Install Homebrew if not already installed
/bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"

# Install required dependencies
brew install openssl pkg-config curl git jq
```

### Windows

1. Install [Windows Subsystem for Linux (WSL)](https://docs.microsoft.com/en-us/windows/wsl/install) with Ubuntu
2. Follow Ubuntu instructions above within WSL
3. Alternatively, install dependencies using [Windows native tools](https://learn.microsoft.com/en-us/windows/dev-environment/)

## Installation Methods

Supernova can be installed using several methods:

1. [Docker (Recommended)](#docker-installation)
2. [Pre-built Binaries](#pre-built-binaries)
3. [Build from Source](#build-from-source)
4. [Package Manager](#package-manager-installation)

Choose the method that best suits your requirements and expertise level.

## Docker Installation

Docker provides the easiest and most consistent installation experience across platforms.

### Prerequisites

- Docker 20.10.0 or later
- Docker Compose 2.0.0 or later

```bash
# Install Docker (Ubuntu/Debian)
curl -fsSL https://get.docker.com -o get-docker.sh
sudo sh get-docker.sh
sudo usermod -aG docker $USER
```

Log out and back in for group changes to take effect.

### Single Node Setup

1. Create a directory for your node:

```bash
mkdir -p ~/supernova
cd ~/supernova
```

2. Download the Docker Compose file:

```bash
curl -L https://raw.githubusercontent.com/supernova/supernova/main/docker/docker-compose.single.yml -o docker-compose.yml
```

3. Create a configuration directory:

```bash
mkdir -p config
curl -L https://raw.githubusercontent.com/supernova/supernova/main/config/node.example.toml -o config/node.toml
```

4. Edit the configuration file:

```bash
nano config/node.toml
```

5. Start the node:

```bash
docker-compose up -d
```

6. Check the logs:

```bash
docker-compose logs -f
```

### Full Network Setup

For a complete Supernova network setup with multiple node types:

```bash
curl -L https://raw.githubusercontent.com/supernova/supernova/main/docker/docker-compose.yml -o docker-compose.yml
docker-compose up -d
```

This will deploy:
- Main blockchain node
- Secondary blockchain node
- Mining node
- Explorer node
- Faucet (testnet only)
- Prometheus monitoring
- Grafana dashboards

## Pre-built Binaries

### Linux (x86_64)

```bash
# Download the latest release
curl -L https://github.com/supernova/supernova/releases/latest/download/supernova-linux-x86_64.tar.gz -o supernova.tar.gz

# Extract the archive
tar -xzf supernova.tar.gz

# Move binaries to a system path
sudo mv supernova/bin/* /usr/local/bin/

# Create configuration directory
mkdir -p ~/.supernova/config
cp supernova/config/node.example.toml ~/.supernova/config/node.toml

# Edit configuration
nano ~/.supernova/config/node.toml

# Start the node
supernova-node
```

### macOS (x86_64/ARM64)

```bash
# Download the latest release
curl -L https://github.com/supernova/supernova/releases/latest/download/supernova-macos-universal.tar.gz -o supernova.tar.gz

# Extract the archive
tar -xzf supernova.tar.gz

# Move binaries to a system path
sudo mv supernova/bin/* /usr/local/bin/

# Create configuration directory
mkdir -p ~/.supernova/config
cp supernova/config/node.example.toml ~/.supernova/config/node.toml

# Edit configuration
nano ~/.supernova/config/node.toml

# Start the node
supernova-node
```

### Windows

1. Download the latest Windows release from the [Supernova GitHub Releases](https://github.com/supernova/supernova/releases/latest)
2. Extract the ZIP file to a location of your choice
3. Open Command Prompt as Administrator
4. Navigate to the extracted directory
5. Run the node:
   ```
   supernova-node.exe --config node.toml
   ```

## Build from Source

Building from source allows for customization and optimization for your specific hardware.

### Requirements

- Rust 1.70.0 or later
- Git
- Build tools (gcc, make, etc.)

### Steps

```bash
# Install Rust if not already installed
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env

# Clone the repository
git clone https://github.com/supernova/supernova.git
cd supernova

# Build the project
cargo build --release

# Create configuration directory
mkdir -p ~/.supernova/config
cp config/node.example.toml ~/.supernova/config/node.toml

# Start the node
./target/release/node
```

## Package Manager Installation

### Ubuntu/Debian APT Repository

```bash
# Add the Supernova repository
echo "deb [signed-by=/usr/share/keyrings/supernova-archive-keyring.gpg] https://apt.supernovanetwork.xyz stable main" | sudo tee /etc/apt/sources.list.d/supernova.list

# Add the repository key
curl -fsSL https://apt.supernovanetwork.xyz/key.gpg | sudo gpg --dearmor -o /usr/share/keyrings/supernova-archive-keyring.gpg

# Update package lists
sudo apt update

# Install Supernova
sudo apt install supernova-node

# Start the service
sudo systemctl enable --now supernova-node
```

### macOS Homebrew

```bash
# Add the Supernova tap
brew tap supernova/supernova

# Install Supernova
brew install supernova-node

# Start the service
brew services start supernova-node
```

## Node Configuration

Supernova uses TOML for configuration. The main configuration file is located at:

- Linux/macOS: `~/.supernova/config/node.toml`
- Windows: `%USERPROFILE%\.supernova\config\node.toml`
- Docker: `./config/node.toml` (mounted volume)

### Basic Configuration Options

Below is a sample configuration file with essential options:

```toml
[node]
# Node identity
node_name = "my-supernova-node"
chain_id = "supernova-mainnet"
environment = "Production"

# Feature flags
metrics_enabled = true
metrics_port = 9090
api_enabled = true
api_port = 9332

[network]
# Network configuration
listen_addr = "0.0.0.0:9333"
max_peers = 50
connection_timeout = 30
enable_upnp = true
enable_peer_exchange = true
enable_nat_traversal = true
max_inbound_connections = 128
max_outbound_connections = 24
reserved_nodes = []
bootstrap_nodes = [
  "node1.supernovanetwork.xyz:9333",
  "node2.supernovanetwork.xyz:9333",
  "node3.supernovanetwork.xyz:9333"
]

[storage]
# Data storage configuration
db_path = "./data"
enable_compression = true
cache_size_mb = 512
backup_interval_hours = 24
enable_pruning = true
pruning_keep_recent = 10000

[mempool]
# Transaction memory pool settings
max_size_mb = 300
min_fee_rate = 1.0
max_tx_per_block = 5000
replace_by_fee = true
max_orphan_tx = 100

[security]
# Security settings
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
# Environmental impact tracking
enable_emissions_tracking = true
enable_treasury = true
enable_green_miner_incentives = true
fee_allocation_percentage = 2.0
rec_incentive_multiplier = 2.0
offset_incentive_multiplier = 1.2

[monitoring]
# Monitoring and metrics
metrics_enabled = true
metrics_endpoint = "0.0.0.0:9091"
enable_system_metrics = true
enable_tracing = true
trace_sampling_rate = 0.1
system_metrics_interval_secs = 15

[mining]
# Mining settings (if running a mining node)
enable_mining = false
mining_threads = 4
mining_address = ""
```

### Advanced Configuration

For advanced users, additional configuration options are available:

```toml
[node]
# Advanced node options
max_block_size_mb = 8
max_orphan_blocks = 100
checkpoint_interval = 10000
checkpoint_verification = true
quantum_signatures_enabled = true

[network]
# Advanced networking
circuit_relay_enabled = true
circuit_relay_hop_limit = 3
max_dial_attempts = 5
min_outbound_per_subnet = 2
persistent_peers = []
private_peers = []

[storage]
# Advanced storage options
db_type = "rocksdb"
index_transactions = true
index_addresses = true
utxo_cache_size_mb = 256
block_cache_size_mb = 128
compression_algorithm = "lz4"
backup_locations = ["local", "s3"]
s3_bucket = "supernova-backups"

[security]
# Advanced security features
allow_local_addresses = false
dns_seed_enforce_diversity = true
private_network_enabled = false
private_network_key = ""
eclipse_protection_enabled = true
eclipse_check_interval_mins = 30
sybil_protection_level = "high"

[consensus]
# Consensus parameters
max_future_block_time_secs = 7200
median_time_span = 11
difficulty_adjustment_window = 2016
target_timespan_seconds = 302400
target_spacing_seconds = 150
min_difficulty = 1.0
retargeting_factor = 0.25

[environmental]
# Advanced environmental settings
emissions_calculation_method = "advanced"
emission_factor_update_interval_days = 7
carbon_intensity_api_endpoint = "https://api.carbonintensity.org.uk/"
carbon_intensity_api_key = ""
environmental_treasury_address = "nova1environmental000000000000000000000000000"

[lightning]
# Lightning Network settings
enable = true
max_channels = 100
default_channel_capacity = 1000000
min_htlc_value_msat = 1000
max_htlc_value_msat = 100000000
use_quantum_signatures = true
watchtower_enabled = true
autopilot_enabled = false
fee_base_msat = 1000
fee_rate_millionths = 1
```

## Node Types

Supernova supports various node types for different use cases:

### Full Node

A complete blockchain node that validates and relays transactions and blocks.

```bash
supernova-node --config ~/.supernova/config/node.toml
```

### Mining Node

A node configured for mining new blocks.

```bash
# Edit config to enable mining
nano ~/.supernova/config/node.toml

# Set mining parameters
# [mining]
# enable_mining = true
# mining_threads = 8
# mining_address = "your-mining-address"

# Start the node
supernova-node --config ~/.supernova/config/node.toml --mine
```

### Explorer Node

A node with additional indexing for block explorer functionality.

```bash
# Edit config to enable explorer features
nano ~/.supernova/config/node.toml

# Set explorer parameters
# [explorer]
# enabled = true
# web_interface_port = 8080
# index_all_transactions = true
# index_addresses = true

# Start the node
supernova-node --config ~/.supernova/config/node.toml --explorer
```

### Light Node

A node with reduced storage requirements that doesn't store the full blockchain.

```bash
# Start in light mode
supernova-node --light
```

## Systemd Service (Linux)

For running Supernova as a system service:

```bash
# Create a systemd service file
sudo nano /etc/systemd/system/supernova-node.service
```

Add the following content:

```
[Unit]
Description=Supernova Blockchain Node
After=network.target

[Service]
User=supernova
Group=supernova
Type=simple
ExecStart=/usr/local/bin/supernova-node --config /home/supernova/.supernova/config/node.toml
Restart=on-failure
RestartSec=10
LimitNOFILE=65536

[Install]
WantedBy=multi-user.target
```

Enable and start the service:

```bash
sudo systemctl daemon-reload
sudo systemctl enable supernova-node
sudo systemctl start supernova-node
```

## Post-Installation Steps

### 1. Verify Installation

Check that your node is running correctly:

```bash
# Check the node status
supernova-cli status

# Check sync progress
supernova-cli sync-status
```

### 2. Open Firewall Ports

Ensure the following ports are open:

```bash
# Allow P2P network traffic
sudo ufw allow 9333/tcp

# Allow RPC interface (only if needed remotely)
sudo ufw allow 9332/tcp

# Allow metrics (only if needed remotely)
sudo ufw allow 9090/tcp
```

### 3. Set Up Monitoring

Enable Prometheus metrics:

```bash
# Edit configuration
nano ~/.supernova/config/node.toml

# Set monitoring parameters
# [monitoring]
# metrics_enabled = true
# metrics_endpoint = "0.0.0.0:9091"
```

### 4. Create a Wallet

Create a wallet for your node:

```bash
supernova-cli wallet create
```

## Upgrading

### Docker Method

```bash
# Pull the latest image
docker-compose pull

# Restart containers
docker-compose up -d
```

### Binary Method

```bash
# Download the latest release
curl -L https://github.com/supernova/supernova/releases/latest/download/supernova-linux-x86_64.tar.gz -o supernova.tar.gz

# Extract the archive
tar -xzf supernova.tar.gz

# Stop the node
sudo systemctl stop supernova-node

# Move binaries to a system path
sudo mv supernova/bin/* /usr/local/bin/

# Start the node
sudo systemctl start supernova-node
```

### Built from Source

```bash
# Change to the source directory
cd supernova

# Pull the latest changes
git pull

# Build the project
cargo build --release

# Stop the node
sudo systemctl stop supernova-node

# Install the updated binaries
cp target/release/node /usr/local/bin/supernova-node
cp target/release/cli /usr/local/bin/supernova-cli

# Start the node
sudo systemctl start supernova-node
```

## Troubleshooting

### Common Issues

#### Node Won't Start

**Symptoms**: Node fails to start, no error messages visible.

**Solutions**:
1. Check logs: `journalctl -u supernova-node -n 100`
2. Verify configuration file exists: `ls -la ~/.supernova/config/node.toml`
3. Check user permissions: `ls -la /usr/local/bin/supernova-node`
4. Check file descriptor limits: `ulimit -n`

#### Node Won't Sync

**Symptoms**: Node starts but doesn't sync with the network.

**Solutions**:
1. Check connectivity: `telnet node1.supernovanetwork.xyz 9333`
2. Verify bootstrap nodes: `grep bootstrap_nodes ~/.supernova/config/node.toml`
3. Check firewall rules: `sudo ufw status`
4. Check disk space: `df -h`

#### High CPU/Memory Usage

**Symptoms**: Node consumes excessive system resources.

**Solutions**:
1. Reduce cache size: `nano ~/.supernova/config/node.toml` (set `cache_size_mb` to a lower value)
2. Enable pruning: Set `enable_pruning = true`
3. Reduce max peers: Set `max_peers` to a lower value (e.g., 30)
4. Disable unnecessary features: Set `metrics_enabled = false`

### Log File Locations

- Systemd service: `journalctl -u supernova-node`
- Binary installation: `~/.supernova/logs/node.log`
- Docker: `docker-compose logs -f`

### Getting Help

- [Supernova Discord Community](https://discord.gg/supernova)
- [Supernova GitHub Issues](https://github.com/supernova/supernova/issues)
- [Supernova Documentation](https://docs.supernovanetwork.xyz)

## Advanced Topics

### Connecting to a Private Network

To connect to a private Supernova network:

```bash
# Edit the configuration file
nano ~/.supernova/config/node.toml

# Set the chain ID and bootstrap nodes
# [node]
# chain_id = "supernova-private-01"
# 
# [network]
# bootstrap_nodes = ["private-node1.example.com:9333"]
# private_network_enabled = true
# private_network_key = "your-private-network-key"
```

### Running Multiple Nodes

To run multiple nodes on the same machine:

```bash
# Create separate configuration files
mkdir -p ~/.supernova/config/node1
mkdir -p ~/.supernova/config/node2

# Copy and edit configuration for each node
cp ~/.supernova/config/node.toml ~/.supernova/config/node1/node.toml
cp ~/.supernova/config/node.toml ~/.supernova/config/node2/node.toml

# Edit each configuration to use different ports and data directories
# Node 1: P2P: 9333, RPC: 9332, Metrics: 9090, DB: ~/.supernova/data/node1
# Node 2: P2P: 9433, RPC: 9432, Metrics: 9190, DB: ~/.supernova/data/node2

# Start each node with its configuration
supernova-node --config ~/.supernova/config/node1/node.toml &
supernova-node --config ~/.supernova/config/node2/node.toml &
```

### Backup and Restore

#### Creating a Backup

```bash
# Manual backup
supernova-cli backup create --output /path/to/backup-directory

# Schedule automatic backups
# [storage]
# backup_interval_hours = 24
# backup_retention_days = 7
```

#### Restoring from Backup

```bash
# Stop the node
sudo systemctl stop supernova-node

# Restore from backup
supernova-cli backup restore --input /path/to/backup-file.tar.gz

# Start the node
sudo systemctl start supernova-node
```

## Conclusion

You have now successfully installed and configured a Supernova blockchain node. Follow the post-installation steps to ensure your node is properly secured and monitored.

For more information, refer to the [Supernova Documentation](https://docs.supernovanetwork.xyz) or join the [Supernova Community](https://discord.gg/supernova) for assistance. 
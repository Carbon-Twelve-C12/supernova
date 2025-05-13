# Supernova Blockchain: Complete Node Setup and Operation Guide

This guide provides detailed instructions for setting up a Supernova blockchain node, participating in mining, and conducting transactions on the network. Supernova combines quantum-resistant cryptography with environmental consciousness for a secure, sustainable blockchain.

## Table of Contents
- [System Requirements](#system-requirements)
- [Quick Start Guide](#quick-start-guide)
- [Detailed Installation](#detailed-installation)
  - [From Source](#from-source)
  - [Using Docker](#using-docker)
- [Node Configuration](#node-configuration)
  - [Basic Configuration](#basic-configuration)
  - [Networking Options](#networking-options)
  - [Storage Settings](#storage-settings)
  - [Advanced Options](#advanced-options)
- [Running Your Node](#running-your-node)
  - [Command Line Options](#command-line-options)
  - [Using Systemd Service](#using-systemd-service)
  - [Node Monitoring](#node-monitoring)
- [Mining Setup](#mining-setup)
  - [Built-in Mining](#built-in-mining)
  - [External Miner Setup](#external-miner-setup)
  - [Mining Pool Configuration](#mining-pool-configuration)
  - [Environmental Reporting](#environmental-reporting)
- [Wallet Operations](#wallet-operations)
  - [Creating a Wallet](#creating-a-wallet)
  - [Securing Your Wallet](#securing-your-wallet)
  - [Quantum-Resistant Addresses](#quantum-resistant-addresses)
- [Transactions](#transactions)
  - [Creating Transactions](#creating-transactions)
  - [Monitoring Transaction Status](#monitoring-transaction-status)
  - [Fee Optimization](#fee-optimization)
- [Lightning Network](#lightning-network)
  - [Setting Up Lightning](#setting-up-lightning)
  - [Opening Channels](#opening-channels)
  - [Lightning Payments](#lightning-payments)
- [Security Best Practices](#security-best-practices)
- [Maintenance and Troubleshooting](#maintenance-and-troubleshooting)
- [Appendix: API Reference](#appendix-api-reference)

## System Requirements

### Minimum Requirements (Development/Testing)
- **CPU**: 4 cores
- **RAM**: 8 GB
- **Storage**: 100 GB SSD
- **Network**: 10 Mbps, stable connection
- **Operating System**: Ubuntu 20.04+, Debian 11+, or CentOS 8+

### Recommended Requirements (Production)
- **CPU**: 16+ cores
- **RAM**: 32+ GB
- **Storage**: 1+ TB NVMe SSD
- **Network**: 100+ Mbps, low-latency connection
- **Operating System**: Ubuntu 22.04 LTS

### Mining Node Requirements
- **CPU**: 16+ cores optimized for mining
- **RAM**: 16+ GB
- **Storage**: 500+ GB NVMe SSD
- **GPU**: Optional but beneficial for mining performance
- **Network**: 100+ Mbps, low-latency connection

## Quick Start Guide

For those familiar with blockchain node operation who want to get started quickly:

```bash
# Clone repository
git clone https://github.com/username/supernova.git
cd supernova

# Build from source
cargo build --release

# Create configuration
mkdir -p ~/.supernova/config
cp config/node.example.toml ~/.supernova/config/node.toml

# Edit configuration (update reward_address for mining)
nano ~/.supernova/config/node.toml

# Start the node
./target/release/node
```

## Detailed Installation

### From Source

Building Supernova from source code provides the most flexibility and ensures you're running the exact code you expect.

1. **Install Dependencies**

```bash
# Ubuntu/Debian
sudo apt update
sudo apt install -y build-essential pkg-config libssl-dev git curl

# CentOS/RHEL
sudo dnf install -y gcc make openssl-devel git curl
```

2. **Install Rust**

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env
```

3. **Clone and Build Supernova**

```bash
git clone https://github.com/username/supernova.git
cd supernova
cargo build --release
```

4. **Create Required Directories**

```bash
mkdir -p ~/.supernova/{config,data,backups}
```

5. **Configure Your Node**

```bash
cp config/node.example.toml ~/.supernova/config/node.toml
nano ~/.supernova/config/node.toml
```

### Using Docker

Docker simplifies deployment and ensures consistent environments across different systems.

1. **Install Docker**

```bash
curl -fsSL https://get.docker.com -o get-docker.sh
sudo sh get-docker.sh
sudo usermod -aG docker $USER
```

2. **Get Supernova Docker Image**

```bash
docker pull supernovachain/node:latest
```

3. **Create Configuration Directory**

```bash
mkdir -p ~/supernova/{config,data,backups}
curl -o ~/supernova/config/node.toml https://raw.githubusercontent.com/username/supernova/main/config/node.example.toml
```

4. **Edit Configuration**

```bash
nano ~/supernova/config/node.toml
```

5. **Run Supernova Node**

```bash
docker run -d --name supernova-node \
  -p 8000:8000 -p 9000:9000 \
  -v ~/supernova/config:/app/config \
  -v ~/supernova/data:/app/data \
  -v ~/supernova/backups:/app/backups \
  supernovachain/node:latest
```

## Node Configuration

### Basic Configuration

Edit `~/.supernova/config/node.toml` with the following essential settings:

```toml
[node]
chain_id = "supernova-mainnet"  # Network to join (mainnet, testnet, etc.)
environment = "Production"      # Development, Testnet, or Production
log_level = "info"              # Logging verbosity (debug, info, warn, error)

[network]
listen_addr = "/ip4/0.0.0.0/tcp/8000"  # Network listening address
max_peers = 50                         # Maximum peer connections

[storage]
db_path = "~/.supernova/data"          # Blockchain database location
```

### Networking Options

For optimal network connectivity:

```toml
[network]
# Add well-known bootstrap nodes
bootstrap_nodes = [
    "/ip4/203.0.113.1/tcp/8000/p2p/QmRZf8wnY2HbQP4h6jtKnHBuEF3V59uCnYx9winHcwUwNX",
    "/ip4/203.0.113.2/tcp/8000/p2p/QmP7HvWHJwJmPWGHH1XtKuKCrFCbjCSRHZ6bA8n5QkRfzC"
]
# For servers behind NAT
external_addr = "/ip4/your.public.ip/tcp/8000"
# For better peer selection
prefer_ipv6 = false
enable_upnp = true
```

### Storage Settings

Optimize storage for performance:

```toml
[storage]
enable_compression = true     # Compress blockchain data
cache_size = 1073741824       # Cache size (1 GB)
max_open_files = 1000         # Database file handles
block_cache_size = 268435456  # Block cache size (256 MB)
```

### Advanced Options

For experienced operators:

```toml
[advanced]
# Enable quantum signature verification
quantum_resistant_verification = true
# Quantum signature scheme (Dilithium, Falcon, Sphincs)
quantum_signature_scheme = "Dilithium"
# Security level (1-5, higher is more secure but slower)
quantum_security_level = 3
```

## Running Your Node

### Command Line Options

Start your node with appropriate options:

```bash
# Basic node start
./target/release/node

# With custom config location
./target/release/node --config /path/to/config.toml

# With block import
./target/release/node --import-blocks /path/to/blocks.dat

# With log level override
./target/release/node --log-level debug

# Start with mining enabled
./target/release/node --mine
```

### Using Systemd Service

For production environments, create a systemd service:

1. **Create Service File**

```bash
sudo nano /etc/systemd/system/supernova.service
```

2. **Add Configuration**

```ini
[Unit]
Description=Supernova Blockchain Node
After=network.target

[Service]
Type=simple
User=YOUR_USERNAME
ExecStart=/path/to/supernova/target/release/node
WorkingDirectory=/home/YOUR_USERNAME
Restart=on-failure
LimitNOFILE=65535

[Install]
WantedBy=multi-user.target
```

3. **Enable and Start Service**

```bash
sudo systemctl daemon-reload
sudo systemctl enable supernova
sudo systemctl start supernova
```

4. **Check Status**

```bash
sudo systemctl status supernova
```

### Node Monitoring

Monitor your node's health and performance:

```bash
# View logs
sudo journalctl -u supernova -f

# Check sync status through API
curl -s http://localhost:9000/api/v1/status | jq

# Monitor system resources
htop -p $(pgrep node)

# Set up Prometheus monitoring
# Access metrics at http://your-node-ip:9000/metrics
```

## Mining Setup

### Built-in Mining

Enable the built-in miner for solo mining:

1. **Update Configuration**

```toml
[mining]
enable = true                     # Enable built-in mining
threads = 8                       # Number of mining threads
reward_address = "snova1..."      # Your Supernova reward address
```

2. **Restart Node**

```bash
sudo systemctl restart supernova
```

3. **Monitor Mining**

```bash
# Check mining status
curl -s http://localhost:9000/api/v1/mining/status | jq

# View mining stats
curl -s http://localhost:9000/api/v1/mining/stats | jq
```

### External Miner Setup

For better mining performance, use a dedicated mining application:

1. **Download Miner**

```bash
git clone https://github.com/username/supernova-miner.git
cd supernova-miner
cargo build --release
```

2. **Configure Miner**

```bash
nano config.toml
```

```toml
[miner]
node_address = "http://localhost:9000"  # Your node's API address
reward_address = "snova1..."            # Your Supernova address
threads = 8                             # Mining threads
algorithm = "quantum-resistant-pow"     # Mining algorithm

# Environmental reporting (optional)
[environmental]
energy_source = "solar"                 # Energy source type
renewable_percentage = 100              # Percentage of renewable energy
region = "US-CA"                        # Your region code
```

3. **Start Miner**

```bash
./target/release/supernova-miner
```

### Mining Pool Configuration

Join a mining pool for more consistent rewards:

1. **Find a Pool**

Choose a reputable Supernova mining pool and register an account.

2. **Configure Pool Mining**

```bash
./target/release/supernova-miner --pool stratum+tcp://pool.example.com:3333 --user username.worker --pass x
```

3. **Monitor Rewards**

Most pools provide a web dashboard to track your mining statistics and rewards.

### Environmental Reporting

Configure your environmental settings for green mining incentives:

1. **Enable Environmental Reporting**

```toml
[environmental]
reporting_enabled = true            # Enable environmental reporting
energy_source = "mixed"             # Energy source type
renewable_percentage = 75           # Percentage from renewable sources
carbon_offset = true                # Whether you use carbon offsets
region = "US-CA"                    # Your region code

# Optional verification details
[environmental.verification]
provider = "GreenCertProvider"      # Verification provider
certificate_id = "GCP-123456"       # Energy certificate ID
verification_url = "https://example.com/verify/GCP-123456"
```

2. **Get Fee Discounts**

Verified green miners may receive fee discounts on the network.

## Wallet Operations

### Creating a Wallet

1. **Generate a New Wallet**

```bash
./target/release/wallet create --output ~/.supernova/wallet.dat
```

2. **Set a Strong Password**

When prompted, enter a strong password to encrypt your wallet.

3. **Backup Your Wallet**

```bash
cp ~/.supernova/wallet.dat ~/.supernova/backups/wallet-$(date +%Y%m%d).dat
```

### Securing Your Wallet

Implement these security practices:

1. **Encrypt Your Wallet**

```bash
./target/release/wallet encrypt ~/.supernova/wallet.dat
```

2. **Create a Paper Backup**

```bash
./target/release/wallet paper-backup ~/.supernova/wallet.dat
```

3. **Use Hardware Security (Optional)**

```bash
./target/release/wallet hardware-setup --ledger
```

### Quantum-Resistant Addresses

Generate and use quantum-resistant addresses:

1. **Generate Address**

```bash
# Generate with Dilithium (default)
./target/release/wallet new-address --type quantum

# Generate with specific algorithm
./target/release/wallet new-address --type quantum --algorithm sphincs
```

2. **List Addresses**

```bash
./target/release/wallet list-addresses
```

## Transactions

### Creating Transactions

Send funds on the Supernova network:

1. **Basic Transaction**

```bash
./target/release/wallet send --to snova1... --amount 5.0
```

2. **Advanced Transaction**

```bash
./target/release/wallet send \
  --to snova1... \
  --amount 5.0 \
  --fee 0.001 \
  --quantum-protected \
  --memo "Payment for services"
```

3. **Batch Transactions**

```bash
./target/release/wallet batch-send transactions.csv
```

### Monitoring Transaction Status

Track your transactions on the network:

1. **Check Transaction**

```bash
./target/release/wallet tx-status <TRANSACTION_ID>
```

2. **Using the Explorer**

Visit the Supernova block explorer at `https://explorer.supernovanetwork.xyz` and enter your transaction ID.

### Fee Optimization

Optimize transaction fees:

1. **Check Recommended Fees**

```bash
./target/release/wallet recommended-fee
```

2. **Set Custom Fee**

```bash
./target/release/wallet send --to snova1... --amount 5.0 --fee 0.00075
```

## Lightning Network

### Setting Up Lightning

Enable fast, off-chain transactions with Lightning:

1. **Initialize Lightning**

```bash
./target/release/lightning init
```

2. **Configure Lightning Node**

```bash
nano ~/.supernova/lightning/config.toml
```

```toml
[lightning]
listen_addr = "0.0.0.0:9735"
announce_addr = "your.public.ip:9735"
alias = "My Supernova LN Node"
color = "008000" # Green color for node visualization

[fees]
base_fee_msat = 1000            # Base fee in millisatoshis
fee_rate = 10                   # Fee rate in parts per million
```

3. **Start Lightning Node**

```bash
./target/release/lightning daemon
```

### Opening Channels

Create payment channels with other nodes:

1. **Connect to Peer**

```bash
./target/release/lightning connect <NODE_ID>@<IP_ADDRESS>:9735
```

2. **Open Channel**

```bash
./target/release/lightning open-channel <NODE_ID> --capacity 0.1 --push-amount 0.02
```

3. **List Channels**

```bash
./target/release/lightning list-channels
```

### Lightning Payments

Make fast, low-fee payments:

1. **Create Invoice**

```bash
./target/release/lightning create-invoice --amount 0.001 --description "Coffee payment"
```

2. **Pay Invoice**

```bash
./target/release/lightning pay <INVOICE>
```

3. **Send Payment**

```bash
./target/release/lightning send-payment <NODE_ID> --amount 0.001 --description "Direct payment"
```

## Security Best Practices

Implement these security measures:

1. **Firewall Configuration**

```bash
# Allow only necessary ports
sudo ufw default deny incoming
sudo ufw default allow outgoing
sudo ufw allow ssh
sudo ufw allow 8000/tcp  # P2P port
sudo ufw allow 9735/tcp  # Lightning (if enabled)
sudo ufw enable
```

2. **Regular Updates**

```bash
# Pull latest updates
cd ~/supernova
git pull
cargo build --release
sudo systemctl restart supernova
```

3. **Secure RPC**

```toml
[rpc]
enabled = true
bind_addr = "127.0.0.1:8545"  # Bind to localhost only
auth_required = true
username = "your_username"
password = "your_strong_password"
```

4. **Use SSL for API Access**

Set up an Nginx reverse proxy with SSL for secure API access.

## Maintenance and Troubleshooting

### Regular Maintenance

Perform these tasks regularly:

1. **Check for Updates**

```bash
cd ~/supernova
git fetch
git log HEAD..origin/main --oneline
```

2. **Backup Data**

```bash
# Backup blockchain data
rsync -av ~/.supernova/data/ ~/backups/supernova-data-$(date +%Y%m%d)/

# Backup wallet
cp ~/.supernova/wallet.dat ~/backups/wallet-$(date +%Y%m%d).dat
```

3. **Verify Database Integrity**

```bash
./target/release/node --verify-db
```

### Common Issues and Solutions

1. **Node Won't Start**

Check logs:
```bash
sudo journalctl -u supernova -n 100
```

2. **Sync Issues**

Try resetting peers:
```bash
./target/release/node --reset-peers
```

3. **Performance Problems**

Adjust cache settings:
```toml
[storage]
cache_size = 2147483648  # Increase to 2 GB
```

4. **Mining Not Working**

Verify reward address:
```bash
./target/release/wallet validate-address <YOUR_REWARD_ADDRESS>
```

## Appendix: API Reference

### Node API

- **Status**: `GET /api/v1/status`
- **Peers**: `GET /api/v1/network/peers`
- **Block Info**: `GET /api/v1/blocks/{height}`
- **Transaction Info**: `GET /api/v1/transactions/{txid}`

### Wallet API

- **Balance**: `GET /api/v1/wallet/balance`
- **New Address**: `POST /api/v1/wallet/address`
- **Send Transaction**: `POST /api/v1/wallet/send`

### Mining API

- **Mining Status**: `GET /api/v1/mining/status`
- **Submit Work**: `POST /api/v1/mining/submit`
- **Environmental Report**: `POST /api/v1/mining/environmental`

### Lightning API

- **Node Info**: `GET /api/v1/lightning/info`
- **Channels**: `GET /api/v1/lightning/channels`
- **Create Invoice**: `POST /api/v1/lightning/invoice`
- **Pay Invoice**: `POST /api/v1/lightning/pay`

---

For more information, visit the [Supernova Documentation](https://docs.supernovanetwork.xyz) or join the community on [Discord](https://discord.gg/supernova). 
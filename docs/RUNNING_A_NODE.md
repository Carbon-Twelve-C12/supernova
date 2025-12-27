# Running a Supernova Node

This guide will help you set up and run your own node. Whether you're contributing to network decentralization or building applications, running a node is straightforward.

**Current Version:** 1.0.0
**Network:** Testnet

---

## Hardware Requirements

Supernova uses quantum-resistant cryptography, which requires more compute power than traditional blockchains.

| Component | Minimum | Recommended |
|-----------|---------|-------------|
| **CPU** | 4 cores | 8+ cores |
| **RAM** | 8 GB | 16-32 GB |
| **Storage** | 100 GB SSD | 200+ GB NVMe |
| **Network** | 100 Mbps | 1 Gbps |

**Storage Note:** Start with at least 100 GB and plan for expansion.

---

## Prerequisites

### Operating System

Ubuntu 22.04 LTS is the officially supported platform. Other Linux distributions may work but are not tested.

### Dependencies

```bash
sudo apt update && sudo apt upgrade -y
sudo apt install -y curl wget jq
```

### Create Service Account

For security, the node runs under a dedicated user account:

```bash
# Create supernova user
sudo useradd -r -m -s /bin/bash supernova

# Create data directories
sudo mkdir -p /data/supernova /etc/supernova /var/log/supernova
sudo chown -R supernova:supernova /data/supernova /var/log/supernova
```

---

## Installation

### Step 1: Download the Binary

```bash
# Set the version
VERSION="1.0.0"

# Download
wget "https://releases.supernovanetwork.xyz/v${VERSION}/supernova-linux-amd64" \
    -O /tmp/supernova
```

### Step 2: Verify the Download

Always verify the checksum before installing. Get the expected checksum from the [releases page](https://github.com/supernova-network/supernova/releases).

```bash
# Verify checksum (replace with actual checksum from releases page)
echo "EXPECTED_SHA256_HERE  /tmp/supernova" | sha256sum -c -
```

### Step 3: Install

```bash
sudo mv /tmp/supernova /usr/local/bin/supernova
sudo chmod +x /usr/local/bin/supernova

# Verify installation
supernova --version
```

### Step 4: Generate Node Identity

Each node needs a unique cryptographic identity:

```bash
sudo -u supernova supernova key generate \
    --output /etc/supernova/node.key
```

**Important:** Back up `/etc/supernova/node.key` securely. This key identifies your node on the network.

---

## Configuration

Create the configuration file at `/etc/supernova/config.toml`:

```bash
sudo nano /etc/supernova/config.toml
```

### Configuration Reference

```toml
# =============================================================================
# SUPERNOVA NODE CONFIGURATION
# =============================================================================

[node]
# Network to connect to: "mainnet" or "testnet"
network = "testnet"

# Where blockchain data is stored
data_dir = "/data/supernova"

# Logging verbosity: "error", "warn", "info", "debug", "trace"
log_level = "info"

# -----------------------------------------------------------------------------
# NETWORK SETTINGS
# -----------------------------------------------------------------------------
[network]
# Address to listen on (0.0.0.0 = all interfaces)
listen_address = "0.0.0.0"

# P2P port - must be accessible from the internet
p2p_port = 8333

# Maximum peer connections
max_peers = 50

# Bootstrap nodes for initial peer discovery
bootnodes = [
    "/dns4/seed.testnet.supernovanetwork.xyz/tcp/8333",
    "/dns4/seed2.testnet.supernovanetwork.xyz/tcp/8333",
    "/dns4/seed3.testnet.supernovanetwork.xyz/tcp/8333"
]

# -----------------------------------------------------------------------------
# RPC API (for wallets and applications)
# -----------------------------------------------------------------------------
[rpc]
# Enable JSON-RPC API
enabled = true

# SECURITY: Bind to localhost only! See "Security" section below.
bind_address = "127.0.0.1"

# RPC port
port = 8545

# -----------------------------------------------------------------------------
# METRICS (optional, for monitoring)
# -----------------------------------------------------------------------------
[metrics]
# Enable Prometheus metrics endpoint
enabled = true

# Metrics bind address
bind_address = "127.0.0.1"

# Metrics port
port = 9615

# -----------------------------------------------------------------------------
# QUANTUM-RESISTANT CRYPTOGRAPHY
# -----------------------------------------------------------------------------
[quantum]
# Enable quantum-resistant signatures
enabled = true

# Algorithm: "dilithium3" (recommended) or "sphincs_sha256_128f"
algorithm = "dilithium3"
```

---

## Starting the Node

### Create Systemd Service

Create the service file at `/etc/systemd/system/supernova.service`:

```bash
sudo nano /etc/systemd/system/supernova.service
```

```ini
[Unit]
Description=Supernova Blockchain Node
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
User=supernova
Group=supernova
ExecStart=/usr/local/bin/supernova --config /etc/supernova/config.toml
Restart=always
RestartSec=5

# Performance tuning
LimitNOFILE=65536
Environment=RUST_LOG=info

# Security hardening
NoNewPrivileges=true
ProtectSystem=strict
ProtectHome=true
ReadWritePaths=/data/supernova /var/log/supernova

[Install]
WantedBy=multi-user.target
```

### Start the Node

```bash
# Reload systemd to recognize the new service
sudo systemctl daemon-reload

# Start the node
sudo systemctl start supernova

# Enable auto-start on boot
sudo systemctl enable supernova

# Check status
sudo systemctl status supernova
```

### Viewing Logs

```bash
# Follow logs in real-time
sudo journalctl -u supernova -f

# View last 100 lines
sudo journalctl -u supernova -n 100

# View logs since last hour
sudo journalctl -u supernova --since "1 hour ago"
```

---

## Verifying Your Node

### Check Node Status

```bash
# Check if the service is running
sudo systemctl status supernova
```

### Check Sync Status

```bash
curl -s http://localhost:8545 \
    -H "Content-Type: application/json" \
    -d '{"jsonrpc":"2.0","method":"eth_syncing","params":[],"id":1}' | jq
```

- Returns `false` when fully synced
- Returns sync progress object while syncing

### Check Peer Count

```bash
curl -s http://localhost:8545 \
    -H "Content-Type: application/json" \
    -d '{"jsonrpc":"2.0","method":"net_peerCount","params":[],"id":1}' | jq
```

A healthy node should have 5+ peers.

### Check Block Height

```bash
curl -s http://localhost:8545 \
    -H "Content-Type: application/json" \
    -d '{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}' | jq
```

Compare with the [block explorer](https://explorer.testnet.supernovanetwork.xyz) to verify sync progress.

---

## Security

### Firewall Configuration

**Required:** Configure a firewall to protect your node.

```bash
# Install UFW if not present
sudo apt install -y ufw

# Default policies
sudo ufw default deny incoming
sudo ufw default allow outgoing

# Allow SSH (adjust port if you use a non-standard port)
sudo ufw allow 22/tcp comment 'SSH'

# Allow P2P connections (required for node operation)
sudo ufw allow 8333/tcp comment 'Supernova P2P'

# Enable firewall
sudo ufw enable

# Verify rules
sudo ufw status
```

### Critical: Do NOT Expose RPC Publicly

The RPC port (8545) should **never** be exposed to the internet. It allows anyone to query your node and can be abused for denial-of-service attacks.

The default configuration binds RPC to `127.0.0.1` (localhost only). Keep it that way unless you have a specific need and proper authentication.

If you need remote RPC access:
- Use SSH tunneling
- Put it behind a reverse proxy with authentication
- Restrict to specific IP addresses

### Keep Your System Updated

```bash
# Enable automatic security updates
sudo apt install unattended-upgrades
sudo dpkg-reconfigure -plow unattended-upgrades
```

---

## Troubleshooting

### Node Won't Start

**Check logs for errors:**
```bash
sudo journalctl -u supernova -n 100 --no-pager
```

**Verify configuration syntax:**
```bash
supernova config validate --config /etc/supernova/config.toml
```

**Check file permissions:**
```bash
ls -la /data/supernova /etc/supernova
# Should be owned by supernova user
```

### No Peers Connecting

**Verify firewall allows P2P port:**
```bash
sudo ufw status
# Should show 8333/tcp ALLOW
```

**Check if port is listening:**
```bash
sudo ss -tlnp | grep 8333
```

**Test connectivity to bootstrap nodes:**
```bash
nc -zv seed.testnet.supernovanetwork.xyz 8333
```

**Check if your ISP blocks the port:**
- Some ISPs block common P2P ports
- Try using a VPS if running from home

### Sync Stuck or Slow

**Check peer count:**
```bash
curl -s http://localhost:8545 \
    -d '{"jsonrpc":"2.0","method":"net_peerCount","params":[],"id":1}'
```

If peers < 3, you may have connectivity issues.

**Restart the node:**
```bash
sudo systemctl restart supernova
```

### High Memory Usage

Add performance limits to your config:

```toml
[performance]
max_mempool_size = 5000
cache_size_mb = 512
```

Then restart:
```bash
sudo systemctl restart supernova
```

### Disk Space Running Low

Check usage:
```bash
df -h /data/supernova
```

Options:
- Add more storage
- Prune old chain data (see documentation)
- Move data directory to larger volume

---

## Getting Help

### Community Resources

- **Discord:** [discord.supernovanetwork.xyz](https://discord.gg/RJ3FpAnG) - Community support
- **Documentation:** [docs.supernovanetwork.xyz](https://docs.supernovanetwork.xyz)
- **Block Explorer:** Coming soon.

### Reporting Issues

Found a bug? Please report it:

1. Check [existing issues](https://github.com/supernova-network/supernova/issues) first
2. Include your node version: `supernova --version`
3. Include relevant logs: `journalctl -u supernova -n 200`
4. Describe steps to reproduce

### Testnet Tokens

Need testnet tokens for development?

- **Faucet:** Coming soon.

---

## Quick Reference

| Command | Description |
|---------|-------------|
| `sudo systemctl start supernova` | Start the node |
| `sudo systemctl stop supernova` | Stop the node |
| `sudo systemctl restart supernova` | Restart the node |
| `sudo systemctl status supernova` | Check status |
| `sudo journalctl -u supernova -f` | Follow logs |
| `supernova --version` | Show version |

| Port | Purpose | Expose? |
|------|---------|---------|
| 8333 | P2P networking | Yes (required) |
| 8545 | JSON-RPC API | No (localhost only) |
| 9615 | Prometheus metrics | No (localhost only) |

---

Thank you for running a Supernova node and contributing to network decentralization!

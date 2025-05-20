# SuperNova Blockchain: Development Guide

## ⚠️ IMPORTANT: DEVELOPMENT STATUS

SuperNova is currently at version 0.6.0 in **ACTIVE DEVELOPMENT** state. This version is **NOT READY** for production deployment:

- **Compilation Issues**: The codebase currently has compilation errors being addressed incrementally
- **Partial Implementation**: Many components are still in development or prototype stage
- **Testing Environment Only**: This guide is for development and testing purposes only

This document provides instructions for setting up a development environment to work on SuperNova.

## Table of Contents

1. [System Requirements](#system-requirements)
2. [Development Environment Setup](#development-environment-setup)
3. [Docker Development Environment](#docker-development-environment)
4. [Testing Framework](#testing-framework)
5. [Troubleshooting](#troubleshooting)

## System Requirements

### Development/Testing Requirements

- **CPU**: 4+ cores
- **RAM**: 8+ GB
- **Storage**: 50+ GB SSD
- **Network**: 10+ Mbps
- **Operating System**: Ubuntu 20.04+, Debian 11+, macOS 10.15+, or Windows 10+ with WSL2

## Development Environment Setup

### Prerequisites

- Rust 1.70.0 or newer
- Git
- Build essentials (gcc, make, etc.)
- OpenSSL development libraries

## Docker Development Environment

### Prerequisites

- Docker 20.10.x or newer
- Docker Compose 2.10.x or newer
- Git
- Basic understanding of Docker concepts

### Single Node Setup

1. **Clone the Repository**

```bash
git clone https://github.com/username/supernova.git
cd supernova
```

2. **Build the Docker Image**

```bash
docker build -t supernova:latest -f docker/Dockerfile .
```

3. **Configure the Node**

```bash
# Copy example configuration
cp config/node.example.toml config/node.toml

# Edit configuration as needed
nano config/node.toml
```

4. **Run a Single Node**

```bash
docker run -d --name supernova-node \
  -p 9333:9333 -p 9332:9332 -p 9090:9090 \
  -v $(pwd)/data:/home/supernova/data \
  -v $(pwd)/config/node.toml:/home/supernova/config/default.toml \
  supernova:latest
```

5. **Check Node Status**

```bash
docker logs -f supernova-node
```

### Multi-Node Setup with Docker Compose

1. **Configure Docker Compose**

```bash
# Create directories for data and logs
mkdir -p data/{node1,node2,miner} logs/{node1,node2,miner}

# Configure network settings for each node in docker-compose.yml
```

2. **Start the Multi-Node Network**

```bash
docker-compose -f docker/docker-compose.yml up -d
```

3. **Monitor Logs**

```bash
docker-compose -f docker/docker-compose.yml logs -f
```

4. **Stop the Network**

```bash
docker-compose -f docker/docker-compose.yml down
```

### Environment Variables

Key environment variables that can be used to configure the Docker containers:

| Variable | Description | Default Value |
|----------|-------------|---------------|
| `RUST_LOG` | Log level (trace, debug, info, warn, error) | info |
| `NODE_NAME` | Name of the node | supernova-node |
| `NETWORK` | Network to connect to (mainnet, testnet) | mainnet |
| `MINE` | Enable mining | false |
| `RUST_BACKTRACE` | Enable backtraces on errors | 0 |
| `SUPERNOVA_CONFIG_DIR` | Configuration directory | /home/supernova/config |
| `SUPERNOVA_DATA_DIR` | Data directory | /home/supernova/data |
| `SUPERNOVA_CHECKPOINTS_DIR` | Checkpoint directory | /home/supernova/checkpoints |
| `SUPERNOVA_BACKUPS_DIR` | Backup directory | /home/supernova/backups |

## Testing Framework

### Prerequisites

- Rust 1.70.0 or newer
- Git
- Build essentials (gcc, make, etc.)
- OpenSSL development libraries

### Setup

1. **Clone the Repository**

```bash
git clone https://github.com/username/supernova.git
cd supernova
```

2. **Build the Project**

```bash
cargo build --release
```

3. **Run Tests**

```bash
cargo test
```

## Troubleshooting

### Common Issues and Solutions

1. **Node Not Syncing**
   - Check network connectivity
   - Verify firewall settings
   - Ensure sufficient disk space

2. **High Resource Usage**
   - Check for abnormal activity
   - Consider increasing resource allocation
   - Optimize configuration parameters

3. **Database Corruption**
   - Stop the node
   - Restore from a backup
   - If no backup is available, resync from scratch

### Diagnostic Commands

```bash
# Check node status
curl -s http://localhost:9332/api/v1/node/status | jq

# View logs
journalctl -u supernova -f

# Check resource usage
top -c -p $(pgrep -f supernova)

# Check network connections
netstat -tuln | grep -E '9333|9332'

# Verify database integrity
supernova check-db --path /home/supernova/data
```

---

This deployment guide provides a comprehensive overview of various deployment options and best practices for the Supernova blockchain. For additional support, please refer to the project documentation or contact the development team. 
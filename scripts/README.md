# Supernova Scripts

This directory contains various utility scripts for building, testing, and running the Supernova blockchain.

## Scripts Overview

### Build and Test Scripts

- **`build_test.sh`** - Tests building and compiling core components. Useful for verifying that the project builds correctly.

- **`test_environmental_features.sh`** - Tests and verifies the environmental features implementation, including quantum signatures.

### Deployment Scripts

- **`docker_setup.sh`** - Sets up and deploys the Supernova testnet using Docker. Creates mock binaries and launches a complete testnet environment.

- **`setup_docker.sh`** - Alternative Docker setup script with additional configuration options.

### Runtime Scripts

- **`run_node.sh`** - Starts a Supernova node with default configuration. Creates necessary directories and uses the example config if none exists.

- **`run_testnet.sh`** - Manages the testnet lifecycle with commands to start, stop, restart, check status, and view logs.

### Utility Scripts

- **`update-repo-urls.sh`** - Updates repository URLs throughout the codebase (used for repository migration).

## Usage Examples

```bash
# Build and test the project
./scripts/build_test.sh

# Set up and run a testnet with Docker
./scripts/docker_setup.sh

# Start a single node
./scripts/run_node.sh

# Manage testnet
./scripts/run_testnet.sh start
./scripts/run_testnet.sh status
./scripts/run_testnet.sh stop

# Test environmental features
./scripts/test_environmental_features.sh
```

## Prerequisites

Most scripts require:
- Docker and Docker Compose (for deployment scripts)
- Rust toolchain (for build scripts)
- Unix-like environment (Linux, macOS, or WSL on Windows)

## Notes

- Always run scripts from the project root directory
- Some scripts may require sudo privileges for Docker operations
- Check individual script headers for specific requirements 
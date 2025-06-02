# SuperNova Testnet Deployment

This directory contains configuration files and scripts for deploying a SuperNova testnet environment. The testnet is designed to provide a sandbox for testing the SuperNova blockchain network before deploying to mainnet.

## Quick Start (Recommended)

We now have a simplified Docker-based setup process that bypasses compilation errors:

```bash
# From the root directory of the project
chmod +x docker_setup.sh
./docker_setup.sh
```

This will:
1. Create mock binaries that simulate the blockchain node
2. Build a Docker image with all required components
3. Launch the testnet with seed nodes, mining nodes, and services
4. Display connection information

For more details on this simplified approach, see [TESTNET_FIXES_UPDATED.md](../../TESTNET_FIXES_UPDATED.md).

## Traditional Setup

If you prefer to build from source (may encounter compilation errors):

```bash
# Build Docker image from source (not recommended)
docker build -t supernova:latest -f docker/Dockerfile .

# Navigate to testnet directory
cd deployments/testnet

# Start the testnet
docker-compose up -d

# Check the status
docker-compose ps
```

## Features

- Multiple seed nodes for network stability
- Mining node for block production
- Faucet for testnet coin distribution
- Monitoring with Prometheus and Grafana
- Web UI for easy interaction

## Configuration

The testnet deployment uses configuration files in the `config/` directory. These files are mounted into the Docker containers and can be modified to change the behavior of the nodes.

## Troubleshooting

If you encounter issues with the testnet deployment, please refer to the [Troubleshooting Guide](../../docs/troubleshooting.md) or the [TESTNET_FIXES_UPDATED.md](../../TESTNET_FIXES_UPDATED.md) document.

## Prerequisites

Before deploying the testnet, ensure you have the following installed:

- Docker (version 20.10.0 or higher)
- Docker Compose (version 2.0.0 or higher)
- Git
- 8+ GB of RAM available for the testnet deployment
- 100+ GB of disk space

## Network Architecture

The testnet deployment includes:

1. **Seed Nodes (2)**
   - Primary network nodes that maintain the blockchain
   - Serve as connection points for other nodes
   - Provide stable network infrastructure

2. **Mining Node (1)**
   - Creates new blocks on the testnet
   - Validates and processes transactions
   - Helps maintain consistent block times

3. **Faucet Node (1)**
   - Distributes test tokens to users
   - Provides a web interface for requesting tokens
   - Tracks distribution history and enforces rate limits

4. **Monitoring Stack**
   - Prometheus for metrics collection
   - Grafana for visualization and dashboards
   - Tracks network health and performance

## Alternative Setup (Manual Method)

If you prefer to set up manually:

1. Clone the SuperNova repository:
   ```bash
   git clone https://github.com/Carbon-Twelve-C12/supernova.git
   cd supernova
   ```

2. Build the SuperNova Docker image:
   ```bash
   docker build -t supernova:latest -f docker/Dockerfile .
   ```

3. Start the testnet using Docker Compose:
   ```bash
   cd deployments/testnet
   docker-compose up -d
   ```

4. Check the status of the deployment:
   ```bash
   docker-compose ps
   ```

## Accessing Services

Once the testnet is running, you can access the following services:

- **Faucet Web Interface**: http://localhost:8080
- **Prometheus**: http://localhost:9090
- **Grafana**: http://localhost:3000 (username: admin, password: supernova)
- **Node RPC**:
  - Seed Node 1: http://localhost:9332
  - Seed Node 2: http://localhost:9335
  - Miner Node: http://localhost:9337
  - Faucet Node: http://localhost:9339

## Configuration

The testnet deployment can be customized by modifying the following files:

- `docker-compose.yml`: Container configuration and networking
- `config/*.toml`: Node-specific configuration files
- `config/prometheus.yml`: Prometheus monitoring configuration
- `config/grafana/`: Grafana dashboards and datasources

### Customizing Network Parameters

To customize the testnet parameters, edit the appropriate configuration files:

1. **Block Time**: Modify `target_block_time_secs` in the node configuration files.
2. **Initial Difficulty**: Change `initial_difficulty` in the node configuration files.
3. **Faucet Amount**: Adjust `distribution_amount` in `config/faucet-node.toml`.

## Using the Testnet

### Creating a Wallet

To create a wallet on the testnet:

```bash
docker exec -it supernova-seed-1 supernova wallet create --network testnet
```

### Requesting Test Tokens

1. Access the faucet web interface at http://localhost:8080
2. Enter your testnet wallet address
3. Complete the captcha verification
4. Submit the request

### Checking Balance

To check your wallet balance:

```bash
docker exec -it supernova-seed-1 supernova wallet balance --address [YOUR_ADDRESS]
```

### Sending Transactions

To send a transaction on the testnet:

```bash
docker exec -it supernova-seed-1 supernova wallet send --address [RECIPIENT_ADDRESS] --amount [AMOUNT] --fee [FEE]
```

## Monitoring

The testnet includes a comprehensive monitoring stack:

1. **Prometheus** (http://localhost:9090):
   - Collects metrics from all nodes
   - Tracks performance, resource usage, and blockchain statistics
   - Provides alerts for potential issues

2. **Grafana** (http://localhost:3000):
   - Visualizes metrics from Prometheus
   - Includes pre-configured dashboards for:
     - Network Overview
     - Block Production
     - Mempool Statistics
     - Node Performance
     - Environmental Metrics

## Running a Public Testnet

To run a public-facing testnet:

1. **Update DNS Seeds**: Add your seed node IPs to the configuration files.
2. **Enable Firewall Rules**: Open ports 9333 (P2P) and 9332 (RPC) for your nodes.
3. **Set Up Domain Names**: Configure DNS records for your nodes.
4. **Deploy Behind a Load Balancer**: For high availability and security.
5. **Set Up SSL Certificates**: Secure API and web interfaces with HTTPS.
6. **Implement Rate Limiting**: Prevent abuse of the faucet and API endpoints.

## Maintenance

### Backup and Recovery

Regular backups are configured by default:
- Blockchain data is backed up daily
- Checkpoints are created hourly
- Backups are stored in the `supernova-backups` volume

To manually create a backup:
```bash
docker-compose exec supernova-backup
```

### Restarting Nodes

To restart specific nodes:
```bash
docker-compose restart seed-node-1 seed-node-2
```

To restart the entire testnet:
```bash
docker-compose down
docker-compose up -d
```

## Contributing

Contributions to the SuperNova testnet configuration are welcome. Please follow these steps:

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Submit a pull request

## License

SuperNova is released under the [MIT License](https://github.com/Carbon-Twelve-C12/supernova/blob/main/LICENSE). 
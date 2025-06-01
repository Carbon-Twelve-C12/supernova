# Supernova Docker Image

This Docker image contains everything needed to run a Supernova blockchain node. It's designed to be easy to deploy and configure for various use cases, including full nodes, mining nodes, and explorer nodes.

## Supported Tags

* `latest` - Latest stable release
* `vX.Y.Z` - Specific version releases (e.g., `v1.0.0`)
* `main` - Latest build from the main branch (may be unstable)

## Quick Start

```bash
# Run a full node with default settings
docker run -d --name supernova-node \
  -p 9333:9333 -p 9332:9332 -p 9090:9090 \
  -v supernova-data:/home/supernova/data \
  mjohnson518/supernova:latest
```

## Configuration

### Environment Variables

The container supports the following environment variables:

| Variable | Description | Default |
|----------|-------------|---------|
| `RUST_LOG` | Log level | `info` |
| `NODE_NAME` | Node name for identification | `node1` |
| `NETWORK` | Network to connect to (`mainnet` or `testnet`) | `testnet` |
| `MINE` | Enable mining | `false` |
| `EXPLORER` | Enable explorer interface | `false` |
| `TZ` | Timezone | `UTC` |

### Volumes

The following volumes are used to persist data:

| Path | Description |
|------|-------------|
| `/home/supernova/data` | Blockchain data |
| `/home/supernova/checkpoints` | Blockchain checkpoints |
| `/home/supernova/backups` | Automated backups |
| `/home/supernova/logs` | Log files |

### Ports

The container exposes the following ports:

| Port | Description |
|------|-------------|
| 9333 | P2P networking |
| 9332 | RPC interface |
| 9090 | Prometheus metrics |
| 8080 | Explorer web interface (when enabled) |

## Usage Examples

### Running a Full Node

```bash
docker run -d --name supernova-node \
  -p 9333:9333 -p 9332:9332 -p 9090:9090 \
  -v supernova-data:/home/supernova/data \
  -v supernova-checkpoints:/home/supernova/checkpoints \
  -v supernova-logs:/home/supernova/logs \
  -e RUST_LOG=info \
  -e NODE_NAME=my-node \
  -e NETWORK=testnet \
  mjohnson518/supernova:latest
```

### Running a Mining Node

```bash
docker run -d --name supernova-miner \
  -p 9333:9333 -p 9332:9332 -p 9090:9090 \
  -v supernova-data:/home/supernova/data \
  -v supernova-checkpoints:/home/supernova/checkpoints \
  -v supernova-logs:/home/supernova/logs \
  -e RUST_LOG=info \
  -e NODE_NAME=my-miner \
  -e NETWORK=testnet \
  -e MINE=true \
  mjohnson518/supernova:latest --mine
```

### Running an Explorer Node

```bash
docker run -d --name supernova-explorer \
  -p 9333:9333 -p 9332:9332 -p 9090:9090 -p 8080:8080 \
  -v supernova-data:/home/supernova/data \
  -v supernova-checkpoints:/home/supernova/checkpoints \
  -v supernova-logs:/home/supernova/logs \
  -e RUST_LOG=info \
  -e NODE_NAME=my-explorer \
  -e NETWORK=testnet \
  -e EXPLORER=true \
  mjohnson518/supernova:latest
```

## Using Docker Compose

For more complex setups, we recommend using Docker Compose. See our [example docker-compose.yml](https://github.com/mjohnson518/supernova/blob/main/docker/docker-compose.yml) file that sets up a complete testnet environment with multiple nodes, miners, and monitoring tools.

### Setting up Environment Variables

1. Copy the example environment file:
   ```bash
   cp .env.example .env
   ```

2. Generate a secure Grafana password:
   ```bash
   openssl rand -base64 32
   ```

3. Edit the `.env` file and set your secure password:
   ```
  
   ```

4. Run Docker Compose:
   ```bash
   docker-compose up -d
   ```

## Health Checks

The image includes a health check that verifies the node is functioning correctly:

```
HEALTHCHECK --interval=30s --timeout=30s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:9332/health || exit 1
```

## Security Considerations

- The container runs as a non-root user (`supernova`) for improved security
- We recommend setting up proper firewall rules to protect your node
- For production deployments, consider using custom configurations with stronger security settings

## Support

For issues, questions, or contributions:

- [GitHub Issues](https://github.com/mjohnson518/supernova/issues)
- [Documentation](https://github.com/mjohnson518/supernova/docs) 
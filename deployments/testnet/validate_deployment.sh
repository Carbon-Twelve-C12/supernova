#!/bin/bash

# Test Supernova Testnet Deployment
set -e

echo "Testing Supernova Testnet Deployment..."
echo "======================================="

# Check if Docker is running
if ! docker info > /dev/null 2>&1; then
    echo "Error: Docker is not running. Please start Docker first."
    exit 1
fi

# Check if docker-compose is available
if ! command -v docker-compose &> /dev/null; then
    echo "Error: docker-compose is not installed."
    exit 1
fi

cd "$(dirname "$0")"

echo ""
echo "1. Checking Docker images can be built..."
echo "------------------------------------------"

# Test build the main node image
echo "Building main node image..."
docker build -t supernova-test:node -f ../../deployment/docker/Dockerfile ../.. || {
    echo "Failed to build main node image"
    exit 1
}

# Test build the oracle image
echo "Building oracle image..."
docker build -t supernova-test:oracle -f ../../deployment/docker/Dockerfile.oracle ../.. || {
    echo "Failed to build oracle image"
    exit 1
}

# Test build the lightning image
echo "Building lightning image..."
docker build -t supernova-test:lightning -f ../../deployment/docker/Dockerfile.lightning ../.. || {
    echo "Failed to build lightning image"
    exit 1
}

# Test build the explorer
echo "Building explorer..."
cd web/explorer
npm install || {
    echo "Failed to install explorer dependencies"
    exit 1
}
cd ../..

# Test build the faucet
echo "Building faucet..."
docker build -t supernova-test:faucet -f web/faucet/Dockerfile web/faucet || {
    echo "Failed to build faucet image"
    exit 1
}

echo ""
echo "2. Validating docker-compose configuration..."
echo "---------------------------------------------"
docker-compose config > /dev/null || {
    echo "docker-compose.yml validation failed"
    exit 1
}

echo ""
echo "3. Creating required config files..."
echo "------------------------------------"

# Create config directory if it doesn't exist
mkdir -p config

# Create a minimal bootstrap node config
if [ ! -f config/bootstrap-node.toml ]; then
    cat > config/bootstrap-node.toml << 'EOF'
[node]
network = "testnet"
data_dir = "/home/supernova/data"
log_dir = "/home/supernova/logs"

[network]
listen_addr = "0.0.0.0:8333"
external_addr = "172.20.0.10:8333"
bootstrap_nodes = []

[rpc]
enabled = true
listen_addr = "0.0.0.0:8332"

[mining]
enabled = false

[environmental]
monitoring_enabled = true
EOF
fi

# Create Prometheus config
if [ ! -f config/prometheus.yml ]; then
    cat > config/prometheus.yml << 'EOF'
global:
  scrape_interval: 15s
  evaluation_interval: 15s

scrape_configs:
  - job_name: 'supernova-nodes'
    static_configs:
      - targets:
          - '172.20.0.10:9100'  # Bootstrap node
          - '172.20.0.11:9100'  # Miner 1
          - '172.20.0.12:9100'  # Miner 2
          - '172.20.0.13:9100'  # Full node
          - '172.20.0.14:9100'  # Lightning node
          - '172.20.0.15:9100'  # Oracle
        labels:
          network: 'testnet'
EOF
fi

echo ""
echo "4. Test Summary"
echo "---------------"
echo "✅ All Docker images can be built successfully"
echo "✅ docker-compose.yml is valid"
echo "✅ Required config files are present"
echo ""
echo "To start the testnet, run:"
echo "  docker-compose up -d"
echo ""
echo "To view logs:"
echo "  docker-compose logs -f"
echo ""
echo "To stop the testnet:"
echo "  docker-compose down"
echo ""
echo "Service URLs (when running):"
echo "  - Bootstrap Node RPC: http://localhost:8332"
echo "  - Block Explorer: http://localhost:3001"
echo "  - Faucet: http://localhost:3002"
echo "  - Grafana Dashboard: http://localhost:3000 (admin/supernova)"
echo "  - Prometheus: http://localhost:9090"
echo "  - Environmental Oracle: http://localhost:8390"
echo "  - Landing Page: http://localhost:8080" 
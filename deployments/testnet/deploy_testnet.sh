#!/bin/bash

# Supernova Testnet Deployment Script
# This script performs all necessary checks and deploys the testnet

set -e

echo "=================================================="
echo "SUPERNOVA TESTNET DEPLOYMENT SCRIPT"
echo "=================================================="
echo ""

# Colors for output
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Function to print colored output
print_status() {
    echo -e "${GREEN}[✓]${NC} $1"
}

print_error() {
    echo -e "${RED}[✗]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[!]${NC} $1"
}

# Check prerequisites
echo "Checking prerequisites..."

# Check Docker
if ! command -v docker &> /dev/null; then
    print_error "Docker is not installed"
    exit 1
else
    print_status "Docker is installed"
fi

# Check Docker Compose
if ! command -v docker-compose &> /dev/null; then
    print_error "Docker Compose is not installed"
    exit 1
else
    print_status "Docker Compose is installed"
fi

# Check if Docker daemon is running
if ! docker info &> /dev/null; then
    print_error "Docker daemon is not running"
    exit 1
else
    print_status "Docker daemon is running"
fi

# Check available disk space (require at least 10GB)
AVAILABLE_SPACE=$(df -BG . | awk 'NR==2 {print $4}' | sed 's/G//')
if [ "$AVAILABLE_SPACE" -lt 10 ]; then
    print_error "Insufficient disk space. At least 10GB required, only ${AVAILABLE_SPACE}GB available"
    exit 1
else
    print_status "Sufficient disk space available (${AVAILABLE_SPACE}GB)"
fi

# Check available memory (require at least 4GB)
AVAILABLE_MEM=$(free -g | awk '/^Mem:/{print $7}')
if [ "$AVAILABLE_MEM" -lt 4 ]; then
    print_warning "Low available memory (${AVAILABLE_MEM}GB). Recommended: 4GB+"
else
    print_status "Sufficient memory available (${AVAILABLE_MEM}GB)"
fi

echo ""
echo "Building Docker images..."

# Build main node image
cd ../..
if docker build -f deployment/docker/Dockerfile -t supernova:testnet .; then
    print_status "Main node image built successfully"
else
    print_error "Failed to build main node image"
    exit 1
fi

# Build Lightning node image (if exists)
if [ -f "deployment/docker/Dockerfile.lightning" ]; then
    if docker build -f deployment/docker/Dockerfile.lightning -t supernova-lightning:testnet .; then
        print_status "Lightning node image built successfully"
    else
        print_warning "Failed to build Lightning node image (continuing anyway)"
    fi
fi

# Build Oracle image (if exists)
if [ -f "deployment/docker/Dockerfile.oracle" ]; then
    if docker build -f deployment/docker/Dockerfile.oracle -t supernova-oracle:testnet .; then
        print_status "Oracle image built successfully"
    else
        print_warning "Failed to build Oracle image (continuing anyway)"
    fi
fi

# Build Explorer image (if exists)
if [ -f "deployment/docker/Dockerfile.explorer" ]; then
    if docker build -f deployment/docker/Dockerfile.explorer -t supernova-explorer:testnet .; then
        print_status "Explorer image built successfully"
    else
        print_warning "Failed to build Explorer image (continuing anyway)"
    fi
fi

cd deployments/testnet

echo ""
echo "Creating configuration files..."

# Create config directory
mkdir -p config/grafana/provisioning/dashboards
mkdir -p config/grafana/provisioning/datasources

# Create Prometheus configuration
cat > config/prometheus.yml << EOF
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

print_status "Prometheus configuration created"

# Create Grafana datasource
cat > config/grafana/provisioning/datasources/prometheus.yml << EOF
apiVersion: 1

datasources:
  - name: Prometheus
    type: prometheus
    access: proxy
    url: http://prometheus:9090
    isDefault: true
EOF

print_status "Grafana configuration created"

# Create data directories
mkdir -p data/{bootstrap,miner1,miner2,fullnode1,lightning,oracle,prometheus,grafana,faucet}
print_status "Data directories created"

echo ""
echo "Starting testnet..."

# Stop any existing containers
docker-compose down 2>/dev/null || true

# Start the testnet
if docker-compose up -d; then
    print_status "Testnet started successfully"
else
    print_error "Failed to start testnet"
    exit 1
fi

echo ""
echo "Waiting for services to initialize..."
sleep 10

# Check service health
echo ""
echo "Checking service health..."

# Function to check if a service is healthy
check_service() {
    local service=$1
    local port=$2
    local endpoint=${3:-"/health"}
    
    if curl -s -f "http://localhost:${port}${endpoint}" > /dev/null 2>&1; then
        print_status "$service is healthy"
        return 0
    else
        print_warning "$service is not responding yet"
        return 1
    fi
}

# Check each service
check_service "Bootstrap node API" 8332
check_service "Miner 1 API" 8342
check_service "Miner 2 API" 8352
check_service "Full node API" 8362
check_service "Prometheus" 9090 "/-/healthy"
check_service "Grafana" 3000 "/api/health"

echo ""
echo "=================================================="
echo "TESTNET DEPLOYMENT COMPLETE!"
echo "=================================================="
echo ""
echo "Access points:"
echo "  - Bootstrap Node API: http://localhost:8332"
echo "  - Block Explorer: http://localhost:3001"
echo "  - Testnet Faucet: http://localhost:3002"
echo "  - Grafana Dashboard: http://localhost:3000 (admin/supernova)"
echo "  - Prometheus: http://localhost:9090"
echo ""
echo "Useful commands:"
echo "  - View logs: docker-compose logs -f [service-name]"
echo "  - Stop testnet: docker-compose down"
echo "  - Clean data: docker-compose down -v"
echo "  - View stats: docker stats"
echo ""
echo "Next steps:"
echo "  1. Wait for nodes to sync (check logs)"
echo "  2. Request test NOVA from faucet"
echo "  3. Monitor network on Grafana dashboard"
echo "  4. Test transactions and mining"
echo ""
print_status "Testnet is ready for testing!" 
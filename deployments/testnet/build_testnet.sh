#!/bin/bash

# Supernova Testnet Build and Deploy Script
# Builds the blockchain node from source

set -e

echo "=================================================="
echo "SUPERNOVA TESTNET BUILD & DEPLOYMENT"
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

# Check if we're in the right directory
if [ ! -f "docker-compose.yml" ]; then
    print_error "Please run this script from the deployments/testnet directory"
    exit 1
fi

echo "Step 1: Building the Supernova node from source..."
echo "This will take 10-20 minutes on first build..."

# Build the Docker image
cd ../..
if docker build -f deployment/docker/Dockerfile -t supernova:latest .; then
    print_status "Docker image built successfully"
else
    print_error "Failed to build Docker image"
    print_warning "This might be due to compilation errors in the Rust code"
    print_warning "Checking for common issues..."
    
    # Try with a more permissive build
    echo ""
    echo "Attempting build with cargo check first..."
    docker run --rm -v $(pwd):/build -w /build rust:1.76-slim-bullseye cargo check
    
    exit 1
fi

cd deployments/testnet

echo ""
echo "Step 2: Creating necessary directories..."

# Create directories for volumes
mkdir -p data/bootstrap data/miner1 data/miner2
mkdir -p logs/bootstrap logs/miner1 logs/miner2
print_status "Data directories created"

echo ""
echo "Step 3: Generating genesis block configuration..."

# Create genesis configuration
cat > config/genesis.json << EOF
{
  "version": 1,
  "network": "testnet",
  "genesis_time": "$(date -u +%Y-%m-%dT%H:%M:%SZ)",
  "chain_id": "supernova-testnet-1",
  "initial_difficulty": "0x1d00ffff",
  "genesis_block": {
    "version": 1,
    "prev_block": "0000000000000000000000000000000000000000000000000000000000000000",
    "merkle_root": "0000000000000000000000000000000000000000000000000000000000000000",
    "timestamp": $(date +%s),
    "bits": "0x1d00ffff",
    "nonce": 0
  },
  "consensus_params": {
    "block_time_seconds": 150,
    "max_block_size": 4194304,
    "initial_reward": 5000000000,
    "halving_interval": 840000
  },
  "environmental_params": {
    "carbon_offset_target": 150,
    "green_mining_bonus": 35,
    "verification_interval_days": 30
  },
  "quantum_params": {
    "algorithm": "dilithium3",
    "hybrid_mode": true
  }
}
EOF

print_status "Genesis configuration created"

echo ""
echo "Step 4: Starting the testnet..."

# Start only essential services first
docker-compose up -d bootstrap-node

echo "Waiting for bootstrap node to initialize..."
sleep 10

# Check if bootstrap node is running
if docker-compose ps bootstrap-node | grep -q "Up"; then
    print_status "Bootstrap node is running"
else
    print_error "Bootstrap node failed to start"
    echo "Checking logs..."
    docker-compose logs bootstrap-node
    exit 1
fi

# Start other services
docker-compose up -d prometheus grafana

echo ""
echo "Step 5: Checking service health..."

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

# Wait for services to be ready
sleep 5

check_service "Bootstrap node RPC" 8332
check_service "Prometheus" 9090 "/-/healthy"
check_service "Grafana" 3000 "/api/health"

echo ""
echo "=================================================="
echo "REAL TESTNET DEPLOYMENT STATUS"
echo "=================================================="
echo ""
echo "Services running:"
docker-compose ps

echo ""
echo "Access points:"
echo "  - RPC Endpoint: http://localhost:8332"
echo "  - P2P Port: 8333"
echo "  - Grafana Dashboard: http://localhost:3000 (admin/supernova)"
echo "  - Prometheus: http://localhost:9090"
echo ""
echo "Useful commands:"
echo "  - Check logs: docker-compose logs -f bootstrap-node"
echo "  - Node CLI: docker exec -it supernova-bootstrap supernova-cli getinfo"
echo "  - Stop testnet: docker-compose down"
echo "  - Clean everything: docker-compose down -v"
echo ""

# Create a simple CLI wrapper
cat > supernova-cli << 'EOF'
#!/bin/bash
docker exec -it supernova-bootstrap supernova-cli "$@"
EOF
chmod +x supernova-cli

print_status "Created supernova-cli wrapper script"
echo ""
echo "You can now use ./supernova-cli to interact with your node"
echo "Example: ./supernova-cli getblockchaininfo"
echo ""
print_status "Real testnet deployment complete!" 
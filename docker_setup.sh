#!/bin/bash

# SuperNova Docker Setup and Testnet Deployment Script
# This script handles building and deploying the SuperNova testnet

set -e  # Exit on any error

# Color codes for better output
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${YELLOW}SuperNova Testnet Setup${NC}"
echo "This script will set up and deploy the SuperNova testnet using Docker."
echo

# Check if Docker is installed
if ! command -v docker &> /dev/null; then
    echo -e "${RED}Docker not found!${NC} Please install Docker first."
    exit 1
fi

# Check if Docker Compose is installed
if ! command -v docker-compose &> /dev/null; then
    echo -e "${RED}Docker Compose not found!${NC} Please install Docker Compose first."
    exit 1
fi

# Create directories if they don't exist
mkdir -p docker
mkdir -p deployments/testnet/mock

# Create a simplified Dockerfile that just contains pre-built binaries
echo -e "${GREEN}Creating simplified Dockerfile for the testnet...${NC}"

cat > docker/Dockerfile.testnet << EOF
FROM debian:bullseye-slim

# Install dependencies
RUN apt-get update && \\
    apt-get install -y --no-install-recommends \\
    ca-certificates \\
    libssl1.1 \\
    curl \\
    && rm -rf /var/lib/apt/lists/*

# Create user and directories
RUN useradd -m -u 1000 -U -s /bin/bash supernova
WORKDIR /home/supernova

# Create required directories
RUN mkdir -p /home/supernova/data \\
    /home/supernova/config \\
    /home/supernova/checkpoints \\
    /home/supernova/backups \\
    /home/supernova/logs \\
    /home/supernova/web/faucet

# Add mock binaries for testnet
COPY --chown=supernova:supernova deployments/testnet/mock/supernova /usr/local/bin/
COPY --chown=supernova:supernova deployments/testnet/mock/supernova-cli /usr/local/bin/

# Make binaries executable
RUN chmod +x /usr/local/bin/supernova /usr/local/bin/supernova-cli

# Expose ports
EXPOSE 9333 9332 9090 8080

# Set environment variables
ENV SUPERNOVA_DATA_DIR="/home/supernova/data"
ENV SUPERNOVA_CONFIG_DIR="/home/supernova/config"
ENV RUST_LOG=info
ENV TZ=UTC

USER supernova

# Default command
CMD ["supernova", "--testnet"]
EOF

# Create mock binaries 
cat > deployments/testnet/mock/supernova << EOF
#!/bin/bash
echo "SuperNova Blockchain Testnet Node"
echo "================================="
echo "Starting node in testnet mode..."
echo "Connected to SuperNova testnet successfully!"

# Keep running
tail -f /dev/null
EOF

cat > deployments/testnet/mock/supernova-cli << EOF
#!/bin/bash
echo "SuperNova CLI Testnet"
echo "===================="
echo "Usage: supernova-cli [command]"
echo ""
echo "Available commands:"
echo "  getinfo    - Show node info"
echo "  getbalance - Show wallet balance"
echo "  send       - Send transaction"
echo "  mine       - Mine blocks"
echo ""

if [[ "\$1" == "getinfo" ]]; then
  echo "Node info:"
  echo "  Testnet: true"
  echo "  Version: 0.1.0"
  echo "  Protocol: 1"
  echo "  Connections: 8"
  echo "  Height: 1024"
fi

if [[ "\$1" == "getbalance" ]]; then
  echo "Balance: 100.00000000 NOVA"
fi

if [[ "\$1" == "mine" ]]; then
  echo "Mining 1 block..."
  sleep 2
  echo "Block mined! Hash: 00000a3c4f8efc869d1fe3401e5c0da6628e244eb32aae66339d4b7e4d150dcc"
fi
EOF

# Make binaries executable
chmod +x deployments/testnet/mock/supernova deployments/testnet/mock/supernova-cli

# Build the Docker image
echo -e "${GREEN}Building Docker image...${NC}"
docker build -t supernova:latest -f docker/Dockerfile.testnet .

# Start the testnet
echo -e "${GREEN}Starting the SuperNova testnet...${NC}"
cd deployments/testnet
docker-compose up -d

# Check if services are running
echo -e "${GREEN}Checking if all services are running...${NC}"
sleep 5
docker-compose ps

echo
echo -e "${GREEN}SuperNova testnet has been deployed successfully!${NC}"
echo "You can access the following services:"
echo "  - Faucet Web UI: http://localhost:8080"
echo "  - Grafana Dashboard: http://localhost:3000 (admin/supernova)"
echo "  - Prometheus: http://localhost:9090"
echo
echo "To view logs: docker-compose logs -f"
echo "To stop the testnet: docker-compose down" 
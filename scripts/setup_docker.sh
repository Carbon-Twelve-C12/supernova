#!/bin/bash

# SuperNova Docker Setup Script
# This script helps set up Docker for the SuperNova testnet

set -e  # Exit on any error

# Color codes for better output
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${YELLOW}SuperNova Docker Setup${NC}"
echo "This script will prepare your Docker environment for running the SuperNova testnet."
echo

# Check if Docker is installed
if ! command -v docker &> /dev/null; then
    echo -e "${RED}Docker not found!${NC} Please install Docker first."
    
    if [[ "$OSTYPE" == "darwin"* ]]; then
        echo "For macOS, please visit: https://docs.docker.com/desktop/install/mac/"
    elif [[ "$OSTYPE" == "linux-gnu"* ]]; then
        echo "For Linux, you can install Docker with:"
        echo "  curl -fsSL https://get.docker.com -o get-docker.sh"
        echo "  sudo sh get-docker.sh"
    elif [[ "$OSTYPE" == "msys" || "$OSTYPE" == "win32" ]]; then
        echo "For Windows, please visit: https://docs.docker.com/desktop/install/windows/"
    fi
    
    exit 1
fi

# Check Docker version
DOCKER_VERSION=$(docker --version | awk '{print $3}' | sed 's/,//')
echo -e "Found Docker version: ${GREEN}$DOCKER_VERSION${NC}"

# Check if Docker daemon is running
if ! docker info &> /dev/null; then
    echo -e "${RED}Docker daemon is not running!${NC}"
    
    if [[ "$OSTYPE" == "darwin"* ]]; then
        echo "Please start Docker Desktop application"
    elif [[ "$OSTYPE" == "linux-gnu"* ]]; then
        echo "Try running: sudo systemctl start docker"
    elif [[ "$OSTYPE" == "msys" || "$OSTYPE" == "win32" ]]; then
        echo "Please start Docker Desktop application"
    fi
    
    exit 1
fi

echo -e "${GREEN}Docker daemon is running${NC}"

# Check Docker Compose
if ! command -v docker-compose &> /dev/null; then
    echo -e "${YELLOW}Warning: docker-compose not found${NC}"
    echo "Docker Compose is required for the testnet. Installing Docker Compose plugin..."
    
    if [[ "$OSTYPE" == "linux-gnu"* ]]; then
        # Linux installation steps
        DOCKER_CONFIG=${DOCKER_CONFIG:-$HOME/.docker}
        mkdir -p $DOCKER_CONFIG/cli-plugins
        curl -SL https://github.com/docker/compose/releases/download/v2.24.1/docker-compose-linux-x86_64 -o $DOCKER_CONFIG/cli-plugins/docker-compose
        chmod +x $DOCKER_CONFIG/cli-plugins/docker-compose
    else
        echo "Please install Docker Compose manually:"
        echo "https://docs.docker.com/compose/install/"
        exit 1
    fi
else
    COMPOSE_VERSION=$(docker-compose --version | awk '{print $4}')
    echo -e "Found Docker Compose version: ${GREEN}$COMPOSE_VERSION${NC}"
fi

# Create required directories
echo "Creating directories for SuperNova testnet..."
mkdir -p data
mkdir -p backups
mkdir -p config

# Ensure correct permissions
if [[ "$OSTYPE" == "linux-gnu"* ]]; then
    echo "Setting directory permissions..."
    chmod -R 755 data backups config
fi

# Build SuperNova Docker image
echo -e "${YELLOW}Building SuperNova Docker image...${NC}"
echo "This might take a while for the first build."

# Navigate to the repository root (assumes script is run from repository root or scripts/ directory)
if [[ $(basename $(pwd)) == "scripts" ]]; then
    cd ..
fi

# Build the Docker image with proper error handling
if ! docker build -t supernova:latest -f docker/Dockerfile .; then
    echo -e "${RED}Docker build failed!${NC} Please check the error message above."
    exit 1
fi

echo -e "${GREEN}Docker image built successfully!${NC}"

# Check if testnet compose file exists
if [ ! -f "deployments/testnet/docker-compose.yml" ]; then
    echo -e "${RED}Error: Could not find docker-compose.yml in deployments/testnet/!${NC}"
    exit 1
fi

echo -e "${GREEN}SuperNova Docker environment is ready!${NC}"
echo
echo "You can now start the testnet with the following commands:"
echo -e "${YELLOW}cd deployments/testnet${NC}"
echo -e "${YELLOW}docker-compose up -d${NC}"
echo
echo "To view logs of the running testnet:"
echo -e "${YELLOW}docker-compose logs -f${NC}"
echo
echo "To stop the testnet:"
echo -e "${YELLOW}docker-compose down${NC}"

# Make the script executable
chmod +x ./scripts/setup_docker.sh 
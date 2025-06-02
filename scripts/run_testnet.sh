#!/bin/bash

# SuperNova Testnet Launcher
# This script simplifies the process of starting and stopping the testnet

function print_header() {
    echo "===================================="
    echo "SuperNova Blockchain Testnet Launcher"
    echo "===================================="
    echo ""
}

function check_prereqs() {
    echo "Checking prerequisites..."
    
    # Check if Docker is installed
    if ! command -v docker &> /dev/null; then
        echo "Error: Docker is not installed"
        echo "Please install Docker before continuing"
        exit 1
    fi
    
    # Check if Docker Compose is installed
    if ! command -v docker-compose &> /dev/null; then
        echo "Error: Docker Compose is not installed"
        echo "Please install Docker Compose before continuing"
        exit 1
    fi
    
    echo "All prerequisites satisfied"
    echo ""
}

function start_testnet() {
    echo "Starting testnet..."
    
    # Create data directories
    mkdir -p data/node1 data/node2 data/node3 data/monitoring
    
    # Start the containers
    docker-compose up -d
    
    echo ""
    echo "Testnet started successfully!"
    echo "Run './scripts/run_testnet.sh status' to check the status"
    echo "Run './scripts/run_testnet.sh logs' to view logs"
    echo ""
}

function stop_testnet() {
    echo "Stopping testnet..."
    
    # Stop the containers
    docker-compose down
    
    echo "Testnet stopped"
    echo ""
}

function show_status() {
    echo "Testnet status:"
    echo ""
    
    docker-compose ps
    
    echo ""
    echo "Network information:"
    echo "--------------------"
    echo "Node 1: http://localhost:9101"
    echo "Node 2: http://localhost:9102"  
    echo "Node 3: http://localhost:9103"
    echo "Monitoring: http://localhost:9900"
    echo ""
}

function show_logs() {
    docker-compose logs -f
}

function show_help() {
    echo "Usage: ./scripts/run_testnet.sh [COMMAND]"
    echo ""
    echo "Commands:"
    echo "  start     Start the testnet"
    echo "  stop      Stop the testnet"
    echo "  restart   Restart the testnet"
    echo "  status    Show testnet status"
    echo "  logs      Show testnet logs"
    echo "  help      Show this help message"
    echo ""
}

# Main script execution
print_header

if [ $# -eq 0 ]; then
    show_help
    exit 0
fi

command=$1

check_prereqs

case $command in
    "start")
        start_testnet
        ;;
    "stop")
        stop_testnet
        ;;
    "restart")
        stop_testnet
        start_testnet
        ;;
    "status")
        show_status
        ;;
    "logs")
        show_logs
        ;;
    "help")
        show_help
        ;;
    *)
        echo "Error: Unknown command '$command'"
        show_help
        exit 1
        ;;
esac 
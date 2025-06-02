#!/bin/bash

# SuperNova Node startup script

# Check if config exists
if [ ! -f "config/node.toml" ]; then
    echo "Configuration file not found!"
    echo "Creating default configuration from template..."
    cp config/node.example.toml config/node.toml
    echo "Please edit config/node.toml with your settings"
    exit 1
fi

# Create directories if they don't exist
mkdir -p data
mkdir -p backups

# Run the node
echo "Starting SuperNova node..."
cargo run --release --bin node -- --with-animation 
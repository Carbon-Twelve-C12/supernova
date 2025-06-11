###############################################################################
# Builder stage - compiles the Supernova blockchain node
###############################################################################
FROM rustlang/rust:nightly-slim as builder

# Install build dependencies
RUN apt-get update && \
    apt-get install -y --no-install-recommends \
    build-essential \
    cmake \
    libssl-dev \
    pkg-config \
    git \
    ca-certificates \
    clang \
    llvm \
    && rm -rf /var/lib/apt/lists/*

# Create build directory
WORKDIR /build

# Copy source code
COPY . /build/

# Build the node binary specifically
WORKDIR /build/node
RUN cargo build --release

###############################################################################
# Runtime stage - minimal image with just the binaries
###############################################################################
FROM debian:bullseye-slim

# Install runtime dependencies
RUN apt-get update && \
    apt-get install -y --no-install-recommends \
    ca-certificates \
    libssl1.1 \
    tzdata \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Create supernova user
RUN useradd -m -u 1000 -U -s /bin/bash supernova

# Copy the node binary - it should be named 'node' based on the Cargo.toml
COPY --from=builder /build/node/target/release/node /usr/local/bin/supernova

# Copy CLI binary if it exists
COPY --from=builder /build/cli/target/release/supernova-cli* /usr/local/bin/ 2>/dev/null || true

# Make binaries executable
RUN chmod +x /usr/local/bin/supernova*

# Create data directories
RUN mkdir -p /home/supernova/data \
    /home/supernova/config \
    /home/supernova/logs \
    && chown -R supernova:supernova /home/supernova

# Switch to supernova user
USER supernova
WORKDIR /home/supernova

# Expose ports
# 8080 - REST API
# 8333 - P2P Network
# 9333 - Lightning Network
EXPOSE 8080 8333 9333

# Volume for blockchain data
VOLUME ["/home/supernova/data"]

# Default command - run in testnet mode
CMD ["supernova", "--testnet", "--data-dir=/home/supernova/data", "--rpc-bind=0.0.0.0:8080", "--p2p-bind=0.0.0.0:8333"] 
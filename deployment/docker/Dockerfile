###############################################################################
# Builder stage - compiles the Supernova blockchain node
###############################################################################
FROM rustlang/rust:nightly-bullseye-slim as builder

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

# Build the node binary from the node directory
WORKDIR /build/node
RUN cargo build --release

# Build the CLI binary from the cli directory  
WORKDIR /build/cli
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

# Copy binaries from builder - they should be in the respective target directories
COPY --from=builder /build/node/target/release/node /usr/local/bin/supernova
COPY --from=builder /build/cli/target/release/supernova-cli /usr/local/bin/supernova-cli

# Make binaries executable
RUN chmod +x /usr/local/bin/supernova /usr/local/bin/supernova-cli

# Create data directories
RUN mkdir -p /home/supernova/data \
    /home/supernova/config \
    /home/supernova/logs \
    && chown -R supernova:supernova /home/supernova

# Switch to supernova user
USER supernova
WORKDIR /home/supernova

# Expose ports
EXPOSE 8332 8333 9100

# Volume for blockchain data
VOLUME ["/home/supernova/data"]

# Default command
CMD ["supernova", "--datadir=/home/supernova/data", "--config=/home/supernova/config/node.toml"] 
###############################################################################
# Builder stage - compiles the SuperNova blockchain node
###############################################################################
FROM rust:1.71-slim-bullseye as builder

# Install build dependencies
RUN apt-get update && \
    apt-get install -y --no-install-recommends \
    build-essential \
    cmake \
    libssl-dev \
    pkg-config \
    git \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Create a new user and switch to it
RUN useradd -m -u 1000 -U -s /bin/bash supernova
USER supernova
WORKDIR /home/supernova

# Copy source code
COPY --chown=supernova:supernova . /home/supernova/supernova

# Build the project with optimizations
WORKDIR /home/supernova/supernova
RUN cargo build --release

###############################################################################
# Runtime stage - uses the compiled binary with minimal dependencies
###############################################################################
FROM debian:bullseye-slim

# Install runtime dependencies
RUN apt-get update && \
    apt-get install -y --no-install-recommends \
    ca-certificates \
    libssl1.1 \
    && rm -rf /var/lib/apt/lists/*

# Create a new user and switch to it
RUN useradd -m -u 1000 -U -s /bin/bash supernova
USER supernova
WORKDIR /home/supernova

# Copy the binary from the builder stage
COPY --from=builder --chown=supernova:supernova /home/supernova/supernova/target/release/supernova /usr/local/bin/
COPY --from=builder --chown=supernova:supernova /home/supernova/supernova/target/release/supernova-cli /usr/local/bin/

# Copy configuration files and docs
COPY --from=builder --chown=supernova:supernova /home/supernova/supernova/config/*.toml /home/supernova/config/
COPY --from=builder --chown=supernova:supernova /home/supernova/supernova/docs /home/supernova/docs/

# Create data directory
RUN mkdir -p /home/supernova/data

# Expose ports
# P2P networking
EXPOSE 9333
# RPC interface
EXPOSE 9332
# Prometheus metrics
EXPOSE 9090

# Default data directory volume
VOLUME ["/home/supernova/data"]

# Set environment variables
ENV SUPERNOVA_DATA_DIR="/home/supernova/data"
ENV SUPERNOVA_CONFIG_DIR="/home/supernova/config"
ENV RUST_BACKTRACE=1
ENV RUST_LOG=info

# Set entrypoint to the supernova binary
ENTRYPOINT ["supernova"]

# Default command (can be overridden)
CMD ["--config", "/home/supernova/config/default.toml"] 
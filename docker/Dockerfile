###############################################################################
# Builder stage - compiles the SuperNova blockchain node
###############################################################################
FROM rust:1.76-slim-bullseye as builder

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
    tzdata \
    curl \
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

# Create required directories
RUN mkdir -p /home/supernova/data \
    /home/supernova/checkpoints \
    /home/supernova/backups \
    /home/supernova/logs

# Expose ports
# P2P networking
EXPOSE 9333
# RPC interface
EXPOSE 9332
# Prometheus metrics
EXPOSE 9090

# Define volumes
VOLUME ["/home/supernova/data"]
VOLUME ["/home/supernova/checkpoints"]
VOLUME ["/home/supernova/backups"]
VOLUME ["/home/supernova/logs"]

# Set environment variables
ENV SUPERNOVA_DATA_DIR="/home/supernova/data"
ENV SUPERNOVA_CONFIG_DIR="/home/supernova/config"
ENV SUPERNOVA_CHECKPOINTS_DIR="/home/supernova/checkpoints"
ENV SUPERNOVA_BACKUPS_DIR="/home/supernova/backups"
ENV SUPERNOVA_LOGS_DIR="/home/supernova/logs"
ENV RUST_BACKTRACE=1
ENV RUST_LOG=info
ENV TZ=UTC

# Health check
HEALTHCHECK --interval=30s --timeout=30s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:9332/health || exit 1

# Create startup script
USER root
RUN echo '#!/bin/bash\n\
mkdir -p "$SUPERNOVA_DATA_DIR" "$SUPERNOVA_CHECKPOINTS_DIR" "$SUPERNOVA_BACKUPS_DIR" "$SUPERNOVA_LOGS_DIR"\n\
chown -R supernova:supernova "$SUPERNOVA_DATA_DIR" "$SUPERNOVA_CHECKPOINTS_DIR" "$SUPERNOVA_BACKUPS_DIR" "$SUPERNOVA_LOGS_DIR"\n\
exec su -s /bin/bash supernova -c "supernova $*"' > /usr/local/bin/docker-entrypoint.sh && \
    chmod +x /usr/local/bin/docker-entrypoint.sh

ENTRYPOINT ["/usr/local/bin/docker-entrypoint.sh"]

# Default command (can be overridden)
CMD ["--config", "/home/supernova/config/default.toml"] 
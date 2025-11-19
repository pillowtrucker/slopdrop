# Slopdrop TCL Evaluation Bot - Podman Container
# Multi-stage build for security and efficiency

# =============================================================================
# Stage 1: Build Environment
# =============================================================================
FROM docker.io/library/rust:1.82-slim-bookworm AS builder

# Install build dependencies
RUN apt-get update && apt-get install -y --no-install-recommends \
    tcl8.6 \
    tcl8.6-dev \
    tclcurl \
    pkg-config \
    git \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Set TCL environment variables for build
ENV PKG_CONFIG_PATH=/usr/lib/x86_64-linux-gnu/pkgconfig:$PKG_CONFIG_PATH
ENV TCL_INCLUDE_PATH=/usr/include/tcl8.6
ENV TCL_LIBRARY=/usr/lib/x86_64-linux-gnu/libtcl8.6.so

# Create build directory
WORKDIR /build

# Copy source code
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY tcl ./tcl

# Build with all frontends for maximum flexibility
RUN cargo build --release --features all-frontends

# =============================================================================
# Stage 2: Runtime Environment (Ubuntu 24.04 - matches dev environment)
# =============================================================================
FROM docker.io/library/ubuntu:24.04 AS runtime

# Install runtime dependencies only
RUN apt-get update && apt-get install -y --no-install-recommends \
    tcl8.6 \
    tclcurl \
    tcllib \
    git \
    ca-certificates \
    openssh-client \
    && rm -rf /var/lib/apt/lists/* \
    && apt-get clean

# Create non-root user for security
RUN groupadd -r slopdrop -g 1000 && \
    useradd -r -g slopdrop -u 1000 -d /app -s /sbin/nologin slopdrop

# Create application directories
RUN mkdir -p /app/state /app/tcl /app/config /app/.ssh && \
    chown -R slopdrop:slopdrop /app && \
    chmod 700 /app/.ssh

WORKDIR /app

# Copy built binary from builder stage
COPY --from=builder /build/target/release/slopdrop /app/slopdrop

# Copy TCL helper scripts
COPY --from=builder /build/tcl /app/tcl

# Set proper permissions
RUN chmod 755 /app/slopdrop && \
    chown -R slopdrop:slopdrop /app

# Switch to non-root user
USER slopdrop

# Configure git for state management (required for commits)
RUN git config --global user.email "slopdrop@container" && \
    git config --global user.name "Slopdrop Bot" && \
    git config --global init.defaultBranch main

# Volume for persistent state (git repository)
VOLUME ["/app/state"]

# Volume for configuration (mount your config.toml here)
VOLUME ["/app/config"]

# Default environment variables
ENV RUST_LOG=info
ENV RUST_BACKTRACE=1

# Health check - verify binary exists and is executable
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD test -x /app/slopdrop || exit 1

# Expose web frontend port (if enabled)
EXPOSE 3000

# Default command - runs IRC frontend with config from volume
# Override with podman run arguments as needed
ENTRYPOINT ["/app/slopdrop"]
CMD ["/app/config/config.toml"]

# =============================================================================
# Labels for container metadata
# =============================================================================
LABEL org.opencontainers.image.title="Slopdrop"
LABEL org.opencontainers.image.description="TCL Evaluation Bot for IRC with multiple frontends"
LABEL org.opencontainers.image.source="https://github.com/pillowtrucker/slopdrop"
LABEL org.opencontainers.image.licenses="AGPL-3.0"

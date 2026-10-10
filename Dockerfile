# Dockerfile for rustok-revisions examples and CLI
FROM rust:1.75-slim-bookworm as builder

# Install dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Set working directory
WORKDIR /app

# Copy manifests
COPY rustok-revisions/Cargo.toml rustok-revisions/
COPY rustok-revisions-derive/Cargo.toml rustok-revisions-derive/
COPY rustok-revisions-cli/Cargo.toml rustok-revisions-cli/

# Create dummy main files for dependency caching
RUN mkdir -p rustok-revisions/src && \
    echo "fn main() {}" > rustok-revisions/src/lib.rs && \
    mkdir -p rustok-revisions-derive/src && \
    echo "fn main() {}" > rustok-revisions-derive/src/lib.rs && \
    mkdir -p rustok-revisions-cli/src && \
    echo "fn main() {}" > rustok-revisions-cli/src/main.rs

# Build dependencies
WORKDIR /app/rustok-revisions-cli
RUN cargo build --release || true

# Copy actual source code
COPY rustok-revisions/ /app/rustok-revisions/
COPY rustok-revisions-derive/ /app/rustok-revisions-derive/
COPY rustok-revisions-cli/ /app/rustok-revisions-cli/

# Build actual binaries
RUN cargo build --release

# Runtime image
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    libssl3 \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Copy binary from builder
COPY --from=builder /app/rustok-revisions-cli/target/release/revctl /usr/local/bin/revctl

# Create non-root user
RUN useradd -m -u 1000 rustok
USER rustok

# Set working directory
WORKDIR /home/rustok

# Default command
ENTRYPOINT ["revctl"]
CMD ["--help"]

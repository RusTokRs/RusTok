# Dockerfile for RusToK Server (Loco backend)
# Supports both development and production builds

# ============================================================================
# Base stage: Install Rust and dependencies
# ============================================================================
FROM rust:1.96-slim AS base

WORKDIR /app

# Install system dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    curl \
    postgresql-client \
    && rm -rf /var/lib/apt/lists/*

# ============================================================================
# Development stage
# ============================================================================
FROM base AS development

# Copy workspace files
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
COPY apps/server ./apps/server

WORKDIR /app/apps/server

# Install cargo-watch for hot reload
RUN cargo install cargo-watch

EXPOSE 5150

# Use cargo-watch for hot reload in dev
CMD ["cargo", "watch", "-x", "run"]

# ============================================================================
# Builder stage: Build production binary
# ============================================================================
FROM base AS builder

# Copy workspace files
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
COPY apps/server ./apps/server

# Build for production
RUN cargo build --release --bin rustok-server

# ============================================================================
# Production stage: Run binary
# ============================================================================
FROM debian:bookworm-slim AS production

WORKDIR /app

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    postgresql-client \
    && rm -rf /var/lib/apt/lists/*

# Copy built binary
COPY --from=builder /app/target/release/rustok-server /app/rustok-server

# Copy migrations and config
COPY apps/server/migration ./migration
COPY apps/server/config ./config

EXPOSE 5150

CMD ["./rustok-server", "start"]

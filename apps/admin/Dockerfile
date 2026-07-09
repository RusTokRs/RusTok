# Dockerfile for Leptos Admin Panel (CSR / WASM)
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
    && rm -rf /var/lib/apt/lists/*

# Install trunk for building Leptos apps
RUN cargo install trunk --locked
RUN cargo install wasm-bindgen-cli --locked

# Add wasm32 target
RUN rustup target add wasm32-unknown-unknown

# ============================================================================
# Development stage
# ============================================================================
FROM base AS development

# Copy workspace files
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
COPY apps/admin ./apps/admin

WORKDIR /app/apps/admin

EXPOSE 3001

# Use trunk serve for hot reload in dev
CMD ["trunk", "serve", "--address", "0.0.0.0", "--port", "3001"]

# ============================================================================
# Builder stage: Build production WASM
# ============================================================================
FROM base AS builder

# Copy workspace files
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
COPY apps/admin ./apps/admin

WORKDIR /app/apps/admin

# Build for production
RUN trunk build --release

# ============================================================================
# Production stage: Serve static files
# ============================================================================
FROM nginx:alpine AS production

# Copy built WASM app
COPY --from=builder /app/apps/admin/dist /usr/share/nginx/html

# Copy nginx config for SPA routing
RUN echo 'server { \
    listen 3001; \
    root /usr/share/nginx/html; \
    index index.html; \
    location / { \
        try_files $uri /index.html; \
    } \
}' > /etc/nginx/conf.d/default.conf

EXPOSE 3001

CMD ["nginx", "-g", "daemon off;"]

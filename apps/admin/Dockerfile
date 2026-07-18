# syntax=docker/dockerfile:1.7

# Development-only CSR host for the standalone Leptos admin surface.
# Production standalone admin must use the nonce-bearing SSR binary; this file
# intentionally exposes no nginx/static "production" target that would bypass
# apps/admin/src/app/security.rs.
FROM node:22-bookworm-slim AS node-runtime
FROM rust:1.96-slim-bookworm AS development

ENV CARGO_INCREMENTAL=0 \
    CARGO_TERM_COLOR=always \
    TRUNK_BUILD_PUBLIC_URL=/ \
    TRUNK_BUILD_LOCKED=true
WORKDIR /workspace

COPY --from=node-runtime /usr/local /usr/local

RUN apt-get update \
    && apt-get install --yes --no-install-recommends \
        ca-certificates \
        curl \
        libssl-dev \
        pkg-config \
    && rm -rf /var/lib/apt/lists/* \
    && rustup target add wasm32-unknown-unknown \
    && CARGO_TARGET_DIR=/tmp/trunk-target \
       cargo install trunk --version 0.21.14 --locked --root /opt/trunk \
    && rm -rf /tmp/trunk-target

COPY . .
WORKDIR /workspace/apps/admin

EXPOSE 3001

CMD ["bash", "-lc", "npm ci --no-audit --no-fund && exec /opt/trunk/bin/trunk serve --address 0.0.0.0 --port 3001"]

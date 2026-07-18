# syntax=docker/dockerfile:1.7

FROM node:22-bookworm-slim AS node-runtime

ARG RUST_VERSION=1.96
ARG DEBIAN_VERSION=bookworm
FROM rust:${RUST_VERSION}-slim-${DEBIAN_VERSION} AS base

ENV CARGO_INCREMENTAL=0 \
    CARGO_TERM_COLOR=always
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

FROM base AS migration

COPY . .
CMD ["cargo", "run", "--locked", "-p", "rustok-migrations", "--bin", "rustok-migrate", "--", "up"]

FROM base AS development

COPY . .
RUN bash scripts/build/build-embedded-admin.sh \
      --tool-root /opt/trunk \
      --target-dir /workspace/target/admin-assets \
      --skip-tool-install
WORKDIR /workspace/apps/server
EXPOSE 5150
CMD ["cargo", "run", "--locked", "-p", "rustok-server", "--bin", "rustok-server"]

FROM base AS builder

ARG SOURCE_DATE_EPOCH=0
ENV SOURCE_DATE_EPOCH=${SOURCE_DATE_EPOCH}
COPY . .
RUN bash scripts/build/build-embedded-admin.sh \
      --tool-root /opt/trunk \
      --target-dir /workspace/target/admin-assets \
      --skip-tool-install \
    && test -s /workspace/apps/admin/dist/index.html \
    && cargo build --locked --release -p rustok-server --bin rustok-server \
    && test -x /workspace/target/release/rustok-server

FROM debian:${DEBIAN_VERSION}-slim AS production

ARG OCI_SOURCE=https://github.com/RusTokRs/RusTok
ARG OCI_VERSION=dev
ARG OCI_REVISION=unknown
LABEL org.opencontainers.image.title="RusToK Server" \
      org.opencontainers.image.description="Modular RusToK API, admin and storefront server" \
      org.opencontainers.image.source="${OCI_SOURCE}" \
      org.opencontainers.image.version="${OCI_VERSION}" \
      org.opencontainers.image.revision="${OCI_REVISION}" \
      org.opencontainers.image.licenses="BUSL-1.1"

ENV RUST_BACKTRACE=1
WORKDIR /app

RUN apt-get update \
    && apt-get install --yes --no-install-recommends \
        ca-certificates \
        libssl3 \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd --system --gid 10001 rustok \
    && useradd --system --uid 10001 --gid 10001 --home-dir /nonexistent --shell /usr/sbin/nologin rustok

COPY --from=builder --chown=10001:10001 /workspace/target/release/rustok-server /app/rustok-server
COPY --from=builder --chown=10001:10001 /workspace/apps/server/config /app/config

USER 10001:10001
EXPOSE 5150
STOPSIGNAL SIGTERM
ENTRYPOINT ["/app/rustok-server"]

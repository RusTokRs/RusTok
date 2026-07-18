# syntax=docker/dockerfile:1.7

ARG RUST_VERSION=1.96
ARG DEBIAN_VERSION=bookworm

FROM rust:${RUST_VERSION}-slim-${DEBIAN_VERSION} AS base

ENV CARGO_INCREMENTAL=0 \
    CARGO_TERM_COLOR=always
WORKDIR /workspace

RUN apt-get update \
    && apt-get install --yes --no-install-recommends \
        ca-certificates \
        libssl-dev \
        pkg-config \
    && rm -rf /var/lib/apt/lists/*

FROM base AS development

COPY . .
WORKDIR /workspace/apps/server
RUN cargo install --locked cargo-watch
EXPOSE 5150
CMD ["cargo", "watch", "-x", "run --locked -p rustok-server --bin rustok-server"]

FROM base AS builder

ARG SOURCE_DATE_EPOCH=0
ENV SOURCE_DATE_EPOCH=${SOURCE_DATE_EPOCH}
COPY . .
RUN cargo build --locked --release -p rustok-server --bin rustok-server \
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

# syntax=docker/dockerfile:1.7

FROM node:22-bookworm-slim AS node-runtime
FROM rust:1.96-slim-bookworm AS admin-toolchain

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
    && CARGO_TARGET_DIR=/tmp/cargo-leptos-target \
       cargo install cargo-leptos --version 0.3.6 --locked --root /opt/cargo-leptos \
    && rm -rf /tmp/trunk-target /tmp/cargo-leptos-target

FROM admin-toolchain AS development

ENV TRUNK_BUILD_PUBLIC_URL=/ \
    TRUNK_BUILD_LOCKED=true
COPY . .
WORKDIR /workspace/apps/admin
EXPOSE 3001
CMD ["bash", "-lc", "npm ci --no-audit --no-fund && exec /opt/trunk/bin/trunk serve --address 0.0.0.0 --port 3001"]

FROM admin-toolchain AS ssr-builder

COPY . .
RUN bash scripts/build/build-standalone-admin-ssr.sh \
      --tool-root /opt/cargo-leptos \
      --target-dir /workspace/target \
      --skip-tool-install \
    && test -x /workspace/target/server/release/rustok-admin \
    && test -s /workspace/target/site/pkg/rustok-admin.css

FROM debian:bookworm-20260713-slim@sha256:7b140f374b289a7c2befc338f42ebe6441b7ea838a042bbd5acbfca6ec875818 AS production

ARG OCI_SOURCE=https://github.com/RusTokRs/RusTok
ARG OCI_VERSION=dev
ARG OCI_REVISION=unknown
LABEL org.opencontainers.image.title="RusToK Standalone Admin" \
      org.opencontainers.image.description="Nonce-bearing Leptos SSR admin host" \
      org.opencontainers.image.source="${OCI_SOURCE}" \
      org.opencontainers.image.version="${OCI_VERSION}" \
      org.opencontainers.image.revision="${OCI_REVISION}" \
      org.opencontainers.image.licenses="BUSL-1.1" \
      org.opencontainers.image.base.name="docker.io/library/debian:bookworm-20260713-slim" \
      org.opencontainers.image.base.digest="sha256:7b140f374b289a7c2befc338f42ebe6441b7ea838a042bbd5acbfca6ec875818"

ENV LEPTOS_OUTPUT_NAME=rustok-admin-app \
    LEPTOS_SITE_ROOT=/app/site \
    LEPTOS_SITE_PKG_DIR=pkg \
    LEPTOS_SITE_ADDR=0.0.0.0:3001 \
    LEPTOS_ENV=PROD \
    RUSTOK_ENV=production \
    RUST_BACKTRACE=1
WORKDIR /app

# RUSTOK_HTTPS is intentionally not set in the image. Production startup fails
# closed until the deployment explicitly declares externally enforced HTTPS.
RUN rm -f /etc/apt/sources.list /etc/apt/sources.list.d/* \
    && printf '%s\n' \
       'deb http://snapshot.debian.org/archive/debian/20260713T000000Z bookworm main' \
       'deb http://snapshot.debian.org/archive/debian/20260713T000000Z bookworm-updates main' \
       'deb http://snapshot.debian.org/archive/debian-security/20260713T000000Z bookworm-security main' \
       > /etc/apt/sources.list \
    && printf '%s\n' 'Acquire::Check-Valid-Until "false";' > /etc/apt/apt.conf.d/99snapshot \
    && apt-get update \
    && apt-get install --yes --no-install-recommends \
        ca-certificates \
        libssl3 \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd --system --gid 10001 rustok \
    && useradd --system --uid 10001 --gid 10001 --home-dir /nonexistent --shell /usr/sbin/nologin rustok

COPY --from=ssr-builder --chown=10001:10001 /workspace/target/server/release/rustok-admin /app/rustok-admin
COPY --from=ssr-builder --chown=10001:10001 /workspace/target/site /app/site

USER 10001:10001
EXPOSE 3001
STOPSIGNAL SIGTERM
ENTRYPOINT ["/app/rustok-admin"]

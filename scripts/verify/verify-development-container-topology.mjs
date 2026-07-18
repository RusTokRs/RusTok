#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(scriptDir, "../..");
const failures = [];

function read(relativePath) {
  const file = path.join(repoRoot, relativePath);
  if (!fs.existsSync(file)) {
    failures.push(`${relativePath}: required file is missing`);
    return "";
  }
  const stats = fs.lstatSync(file);
  if (!stats.isFile() || stats.isSymbolicLink()) {
    failures.push(`${relativePath}: must be a regular non-symlink file`);
    return "";
  }
  return fs.readFileSync(file, "utf8");
}

function requireMarkers(relativePath, markers) {
  const source = read(relativePath);
  for (const marker of markers) {
    if (!source.includes(marker)) failures.push(`${relativePath}: missing marker ${marker}`);
  }
  return source;
}

function forbidMarkers(relativePath, markers) {
  const source = read(relativePath);
  for (const marker of markers) {
    if (source.includes(marker)) failures.push(`${relativePath}: forbidden marker ${marker}`);
  }
}

function requireCount(relativePath, marker, expected) {
  const actual = read(relativePath).split(marker).length - 1;
  if (actual !== expected) {
    failures.push(`${relativePath}: expected ${expected} occurrence(s) of ${marker}, found ${actual}`);
  }
}

requireMarkers("apps/admin/Dockerfile", [
  "Development-only CSR host",
  "Production standalone admin must use the nonce-bearing SSR binary",
  "FROM node:22-bookworm-slim AS node-runtime",
  "FROM rust:1.96-slim-bookworm AS development",
  "COPY --from=node-runtime /usr/local /usr/local",
  "TRUNK_BUILD_PUBLIC_URL=/",
  "TRUNK_BUILD_LOCKED=true",
  "cargo install trunk --version 0.21.14 --locked --root /opt/trunk",
  "COPY . .",
  "npm ci --no-audit --no-fund",
  "exec /opt/trunk/bin/trunk serve --address 0.0.0.0 --port 3001",
]);
forbidMarkers("apps/admin/Dockerfile", [
  "AS production",
  "nginx",
  "cargo install trunk --locked\n",
  "cargo install wasm-bindgen-cli",
  "trunk build --release",
  "COPY crates ./crates",
  "COPY apps/admin ./apps/admin",
]);

requireMarkers("apps/server/Dockerfile", [
  "FROM node:22-bookworm-slim AS node-runtime",
  "FROM rust:${RUST_VERSION}-slim-${DEBIAN_VERSION} AS base",
  "curl",
  "cargo install trunk --version 0.21.14 --locked --root /opt/trunk",
  "bash scripts/build/build-embedded-admin.sh",
  "--skip-tool-install",
  'CMD ["cargo", "run", "--locked", "-p", "rustok-server", "--bin", "rustok-server"]',
  "test -s /workspace/apps/admin/dist/index.html",
  "cargo build --locked --release -p rustok-server --bin rustok-server",
]);
requireCount("apps/server/Dockerfile", "bash scripts/build/build-embedded-admin.sh", 2);
forbidMarkers("apps/server/Dockerfile", [
  "cargo-watch",
  "cargo install trunk --locked\n",
  'CMD ["cargo", "watch"',
  "COPY crates ./crates",
  "COPY apps/server ./apps/server",
]);

requireMarkers("docker-compose.full-dev.yml", [
  "The Leptos storefront is an SSR library embedded in `rustok-server`",
  "Backend Server (Axum + embedded Leptos admin/storefront)",
  "context: .\n      dockerfile: apps/server/Dockerfile\n      target: development",
  "./apps/server:/workspace/apps/server",
  "./crates:/workspace/crates",
  "./modules.toml:/workspace/modules.toml:ro",
  "curl --fail --silent http://localhost:5150/health",
  "Standalone Leptos Admin (development CSR host only)",
  "dockerfile: apps/admin/Dockerfile",
  "./apps/admin:/workspace/apps/admin",
  "admin_node_modules:/workspace/apps/admin/node_modules",
  "The Rust/Leptos storefront is served by `server`",
]);
forbidMarkers("docker-compose.full-dev.yml", [
  "/app/apps/server",
  "/app/crates",
  "http://localhost:5150/api/health",
  "storefront-leptos:",
  "apps/storefront/Dockerfile",
  "rustok_storefront_leptos",
  '"3101:3101"',
  "nginx:alpine",
]);

requireMarkers(".github/workflows/hardening-gates.yml", [
  "Verify development container topology",
  "verify-development-container-topology.mjs",
]);
requireMarkers("scripts/verify/verify-all.sh", [
  "development-container-topology  Verify truthful Rust development container topology",
  "verify-development-container-topology.mjs:Development Container Topology",
]);

if (failures.length > 0) {
  console.error("Development container topology verification failed:");
  failures.forEach((failure) => console.error(`✗ ${failure}`));
  process.exit(Math.min(failures.length, 255));
}

console.log(
  "✔ development containers use locked admin tooling, real health paths, /workspace mounts and server-hosted Rust storefront topology",
);

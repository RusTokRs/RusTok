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
  "FROM nginx:",
  'CMD ["nginx"',
  "/usr/share/nginx/html",
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
  "FROM base AS migration",
  'CMD ["cargo", "run", "--locked", "-p", "rustok-migrations", "--bin", "rustok-migrate", "--", "up"]',
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

requireMarkers("crates/rustok-migrations/Cargo.toml", [
  'name = "rustok-migrate"',
  'path = "src/bin/rustok_migrate.rs"',
  "tokio.workspace = true",
]);
requireMarkers("crates/rustok-migrations/src/bin/rustok_migrate.rs", [
  "enum Command",
  "Up,",
  "Status,",
  'command == "up"',
  'command == "status"',
  "only up and status are allowed",
  'env::var("DATABASE_URL")',
  "Migrator::up(&database, None).await?",
  "Migrator::get_pending_migrations(&database).await?",
  'for forbidden in ["down", "fresh", "reset", "refresh"]',
]);
forbidMarkers("crates/rustok-migrations/src/bin/rustok_migrate.rs", [
  "Migrator::down",
  "Migrator::fresh",
  "Migrator::reset",
  "Migrator::refresh",
  "Command::Down",
  "Command::Fresh",
]);

const postgresDigest =
  "postgres:16-bookworm@sha256:05bb94c3949035f4da16815d91b389443f3dbc5db95d65e2cb9b1abbf8565974";
requireMarkers("docker-compose.yml", [
  postgresDigest,
  "POSTGRES_DB: postgres",
  "POSTGRES_USER: postgres",
  "POSTGRES_PASSWORD: postgres",
  "db-bootstrap:",
  "condition: service_healthy",
  "CREATE ROLE rustok",
  "NOSUPERUSER",
  "NOCREATEDB",
  "NOCREATEROLE",
  "NOREPLICATION",
  "NOBYPASSRLS",
  "CONNECTION LIMIT 20",
  "ALTER ROLE rustok WITH",
  "createdb --host db --username postgres --owner rustok rustok_dev",
  "REVOKE CREATE ON SCHEMA public FROM PUBLIC",
  "GRANT ALL ON SCHEMA public TO rustok",
]);
requireCount("docker-compose.yml", postgresDigest, 2);
forbidMarkers("docker-compose.yml", [
  "image: postgres:16-alpine",
  "POSTGRES_USER: rustok",
  "POSTGRES_DB: rustok_dev",
  "CREATE ROLE rustok SUPERUSER",
]);

requireMarkers("docker-compose.full-dev.yml", [
  "The Leptos storefront is an SSR library embedded in `rustok-server`",
  "One-shot schema migration. The CLI intentionally supports only up/status.",
  "migrate:",
  "target: migration",
  "container_name: rustok_migrate",
  "DATABASE_URL: postgres://rustok:rustok@db:5432/rustok_dev",
  "db-bootstrap:\n        condition: service_completed_successfully",
  "Backend Server (Axum + embedded Leptos admin/storefront)",
  "context: .\n      dockerfile: apps/server/Dockerfile\n      target: development",
  "migrate:\n        condition: service_completed_successfully",
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
requireCount("docker-compose.full-dev.yml", "condition: service_completed_successfully", 2);
forbidMarkers("docker-compose.full-dev.yml", [
  "/app/apps/server",
  "/app/crates",
  "http://localhost:5150/api/health",
  "storefront-leptos:",
  "apps/storefront/Dockerfile",
  "rustok_storefront_leptos",
  '"3101:3101"',
  "nginx:alpine",
  "target: development\n    container_name: rustok_migrate",
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
  "✔ development containers use bounded PostgreSQL ownership, non-destructive migrations, locked admin tooling, real health paths and server-hosted Rust storefront topology",
);

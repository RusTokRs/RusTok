#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

function parseArguments(argv) {
  const options = {};
  for (let index = 0; index < argv.length; index += 1) {
    if (argv[index] === "--root") {
      const value = argv[index + 1];
      if (!value) throw new Error("--root requires a value");
      options.root = value;
      index += 1;
      continue;
    }
    throw new Error(`unknown argument: ${argv[index]}`);
  }
  return options;
}

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const options = parseArguments(process.argv.slice(2));
const repoRoot = path.resolve(options.root || path.resolve(scriptDir, "../.."));
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
}

function forbidMarkers(relativePath, markers) {
  const source = read(relativePath);
  for (const marker of markers) {
    if (source.includes(marker)) failures.push(`${relativePath}: forbidden marker ${marker}`);
  }
}

requireMarkers("apps/admin/Cargo.toml", [
  '[[bin]]\nname = "rustok-admin"',
  '[package.metadata.leptos]',
  'output-name = "rustok-admin"',
  'site-root = "target/site"',
  'site-pkg-dir = "pkg"',
  'bin-features = ["ssr"]',
  'bin-default-features = false',
  'bin-cargo-args = ["--locked"]',
  'lib-features = ["hydrate"]',
  'lib-default-features = false',
  'lib-cargo-args = ["--locked"]',
]);
forbidMarkers("apps/admin/Cargo.toml", [
  'name = "rustok-admin-app"\npath = "src/main.rs"',
  'site-root = "target/admin-site"',
  'bin-cargo-args = []',
  'lib-cargo-args = []',
]);

requireMarkers("scripts/build/build-standalone-admin-ssr.sh", [
  "set -euo pipefail",
  "cargo install cargo-leptos --version 0.3.6 --locked --root",
  'CARGO_TARGET_DIR="$tool_root/target"',
  "rustup target add wasm32-unknown-unknown",
  'npm ci --prefix "$repo_root/apps/admin" --no-audit --no-fund',
  'site_root="$target_dir/site"',
  'server_binary="$target_dir/release/rustok-admin"',
  'LEPTOS_SITE_ROOT="$site_root"',
  "cargo leptos build --release -p rustok-admin",
  'TRUNK_STAGING_DIR="$site_pkg"',
  'mv "$site_pkg/output.css" "$site_pkg/rustok-admin.css"',
  "standalone SSR JavaScript hydration artifact is missing",
  "standalone SSR WebAssembly hydration artifact is missing",
  'href="/pkg/rustok-admin.css"',
]);
forbidMarkers("scripts/build/build-standalone-admin-ssr.sh", [
  "cargo leptos build --release --locked",
  "target/server/release/rustok-admin",
  "cargo install cargo-leptos --locked\n",
  "|| true",
  "eval ",
]);

requireMarkers("apps/admin/Dockerfile", [
  "FROM rust:1.96-slim-bookworm AS admin-toolchain",
  "cargo install trunk --version 0.21.14 --locked --root /opt/trunk",
  "cargo install cargo-leptos --version 0.3.6 --locked --root /opt/cargo-leptos",
  "FROM admin-toolchain AS ssr-builder",
  "build-standalone-admin-ssr.sh",
  "test -x /workspace/target/release/rustok-admin",
  "test -s /workspace/target/site/pkg/rustok-admin.css",
  "FROM debian:bookworm-20260713-slim@sha256:7b140f374b289a7c2befc338f42ebe6441b7ea838a042bbd5acbfca6ec875818 AS production",
  'org.opencontainers.image.description="Nonce-bearing Leptos SSR admin host"',
  "snapshot.debian.org/archive/debian/20260713T000000Z",
  'Acquire::Check-Valid-Until "false";',
  "LEPTOS_OUTPUT_NAME=rustok-admin",
  "LEPTOS_SITE_ROOT=/app/site",
  "LEPTOS_SITE_PKG_DIR=pkg",
  "LEPTOS_SITE_ADDR=0.0.0.0:3001",
  "LEPTOS_ENV=PROD",
  "RUSTOK_ENV=production",
  "RUSTOK_HTTPS is intentionally not set in the image",
  "--uid 10001 --gid 10001",
  "COPY --from=ssr-builder --chown=10001:10001 /workspace/target/release/rustok-admin /app/rustok-admin",
  "COPY --from=ssr-builder --chown=10001:10001 /workspace/target/site /app/site",
  "USER 10001:10001",
  'ENTRYPOINT ["/app/rustok-admin"]',
]);
forbidMarkers("apps/admin/Dockerfile", [
  "FROM nginx:",
  "/usr/share/nginx/html",
  "RUSTOK_HTTPS=true",
  "RUSTOK_HTTPS=1",
  "LEPTOS_OUTPUT_NAME=rustok-admin-app",
  "/workspace/target/server/release/rustok-admin",
  "FROM debian:bookworm-slim",
  "deb.debian.org",
  "security.debian.org",
  "USER root",
  "|| true",
]);

requireMarkers("apps/admin/src/main.rs", [
  'routing::{get, post}',
  '.route("/health", get(|| async { StatusCode::OK }))',
  "validate_admin_security_profile().expect",
  ".layer(middleware::from_fn(admin_security_headers))",
  "provide_context(nonce)",
]);
requireMarkers("apps/admin/src/app/security.rs", [
  'path.starts_with("/api/") || path == "/health" || path.starts_with("/health/")',
  "RUSTOK_HTTPS must be set to true for the standalone production admin host",
  "style-src-attr 'none'",
  "standalone_admin_api_and_health_policies_are_scriptless",
  "standalone_admin_health_uses_scriptless_policy",
  "assert_eq!(select_csp(\"/health\", None, false), API_CSP)",
]);
forbidMarkers("apps/admin/src/app/security.rs", [
  "style-src-attr 'unsafe-inline'",
  "script-src 'self' 'unsafe-inline'",
]);
requireMarkers("apps/admin/src/app/shell.rs", [
  'href="/pkg/rustok-admin.css"',
  "MetaTags",
]);

requireMarkers(".github/workflows/hardening-gates.yml", [
  "Verify standalone admin SSR contract",
  "verify-standalone-admin-ssr-contract.mjs",
]);
requireMarkers("scripts/verify/verify-all.sh", [
  "standalone-admin-ssr-contract  Verify nonce-bearing standalone admin production contract",
  "verify-standalone-admin-ssr-contract.mjs:Standalone Admin SSR Contract",
]);
requireMarkers(".github/workflows/release-infrastructure.yml", [
  "Verify standalone admin SSR with base-owned policy",
  "base/scripts/verify/verify-standalone-admin-ssr-contract.mjs",
  "Verify explicitly approved standalone admin SSR policy",
  "head/scripts/verify/verify-standalone-admin-ssr-contract.mjs",
  '--root "$GITHUB_WORKSPACE/head"',
]);
requireMarkers("scripts/verify/verify-release-infrastructure-approval.mjs", [
  "apps/admin/Dockerfile",
  "apps/admin/src/main.rs",
  "apps/admin/src/app/security.rs",
  "apps/admin/src/app/shell.rs",
  "scripts/build/build-standalone-admin-ssr.sh",
  "scripts/verify/verify-standalone-admin-ssr-contract.mjs",
]);

if (failures.length > 0) {
  console.error(`Standalone admin SSR contract verification failed for ${repoRoot}:`);
  failures.forEach((failure) => console.error(`✗ ${failure}`));
  process.exit(Math.min(failures.length, 255));
}

console.log(
  `✔ cargo-leptos 0.3.6, locked Cargo inputs, nonce-bearing SSR runtime, scriptless health CSP and non-root pinned image are bound in ${repoRoot}`,
);

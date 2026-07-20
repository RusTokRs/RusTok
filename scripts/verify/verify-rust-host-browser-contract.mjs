#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

const ACTIONS = Object.freeze({
  checkout: "actions/checkout@9c091bb21b7c1c1d1991bb908d89e4e9dddfe3e0",
  setupNode: "actions/setup-node@249970729cb0ef3589644e2896645e5dc5ba9c38",
  uploadArtifact: "actions/upload-artifact@043fb46d1a93c77aae656e7c1c64a875d1fc6a0a",
});

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

function actionReferences(relativePath) {
  return [...read(relativePath).matchAll(/^\s*uses:\s*([^\s#]+)(?:\s+#.*)?\s*$/gm)].map(
    (match) => match[1],
  );
}

function requireActionCounts(relativePath, expectedCounts) {
  const references = actionReferences(relativePath);
  for (const reference of references) {
    if (!Object.values(ACTIONS).includes(reference)) {
      failures.push(`${relativePath}: unapproved or unpinned action ${reference}`);
    }
  }
  for (const [reference, expected] of expectedCounts) {
    const actual = references.filter((candidate) => candidate === reference).length;
    if (actual !== expected) {
      failures.push(`${relativePath}: expected ${expected} use(s) of ${reference}, found ${actual}`);
    }
  }
}

const workflow = ".github/workflows/rust-host-browser-smoke.yml";
requireMarkers(workflow, [
  "name: Rust-hosted Browser Smoke",
  "permissions:\n  contents: read",
  "runs-on: ubuntu-24.04",
  "image: postgres:16-bookworm@sha256:05bb94c3949035f4da16815d91b389443f3dbc5db95d65e2cb9b1abbf8565974",
  "Create bounded PostgreSQL smoke role",
  "CREATEDB",
  "NOSUPERUSER",
  "NOCREATEROLE",
  "NOREPLICATION",
  "NOBYPASSRLS",
  "CONNECTION LIMIT 10",
  "RUSTOK_MIGRATION_SMOKE_ADMIN_URL: postgres://${{ env.RUSTOK_BROWSER_DATABASE_ROLE }}",
  "RUSTOK_MIGRATION_SMOKE_KEEP_DB: \"1\"",
  "postgres_zero_migration_smoke_applies_from_empty_database",
  "scripts/build/build-embedded-admin.sh",
  "--public-url /admin/",
  "npm ci --prefix apps/next-admin --no-audit --no-fund",
  "npx --no-install playwright install --with-deps chromium",
  "RUSTOK_ENV: browser-smoke",
  "SUPERADMIN_TENANT_SLUG: default",
  "cargo run --locked -p rustok-server --bin rustok-server",
  "curl --fail --silent --show-error http://127.0.0.1:5150/health",
  "playwright.rust-hosts.config.ts",
  "Upload Rust-host browser evidence",
  "if: always()",
  "DROP DATABASE IF EXISTS ${RUSTOK_BROWSER_DATABASE} WITH (FORCE)",
  "DROP ROLE IF EXISTS ${RUSTOK_BROWSER_DATABASE_ROLE}",
]);
forbidMarkers(workflow, [
  "permissions:\n  contents: write",
  "packages: write",
  "id-token: write",
  "secrets:",
  "continue-on-error:",
  "runs-on: ubuntu-latest",
  "image: postgres:16\n",
  "postgres://postgres:postgres@",
  "RUSTOK_MIGRATION_SMOKE_ADMIN_URL: postgres://postgres",
  "cargo run -p rustok-server",
  "npx playwright test",
  "npm install",
  "actions/checkout@v",
  "actions/setup-node@v",
  "actions/upload-artifact@v",
]);
requireActionCounts(
  workflow,
  new Map([
    [ACTIONS.checkout, 1],
    [ACTIONS.setupNode, 1],
    [ACTIONS.uploadArtifact, 1],
  ]),
);

const nextWorkflow = ".github/workflows/browser-e2e.yml";
requireMarkers(nextWorkflow, [
  "name: Browser E2E",
  "permissions:\n  contents: read",
  "runs-on: ubuntu-24.04",
  "apps/next-admin",
  "apps/next-frontend",
  "fail-fast: false",
  "persist-credentials: false",
  "npm ci --no-audit --no-fund",
  "npx --no-install playwright install --with-deps chromium",
  "npm run test:e2e",
  "Upload Playwright diagnostics",
  "if: failure()",
]);
forbidMarkers(nextWorkflow, [
  "permissions:\n  contents: write",
  "continue-on-error:",
  "runs-on: ubuntu-latest",
  "npm install",
  "npx playwright install",
  "actions/checkout@v",
  "actions/setup-node@v",
  "actions/upload-artifact@v",
]);
requireActionCounts(
  nextWorkflow,
  new Map([
    [ACTIONS.checkout, 1],
    [ACTIONS.setupNode, 1],
    [ACTIONS.uploadArtifact, 1],
  ]),
);

requireMarkers("apps/server/config/browser-smoke.yaml", [
  "profile: multi_tenant",
  "header_name: x-tenant-slug",
  "fallback_mode: disabled",
  "enabled: false\n      driver: memory",
  "search_indexing: false",
  "rate_limit:\n      enabled: false\n      backend: memory",
  "transport: memory",
  "relay_target: memory",
  "email:\n      enabled: false\n      provider: none",
  "workflow_cron_enabled: false",
  "seo_bulk_enabled: false",
]);
forbidMarkers("apps/server/config/browser-smoke.yaml", [
  "meilisearch",
  "redis://",
  "transport: iggy",
  "backend: redis",
  "fallback_mode: default_tenant",
  "X-Tenant-ID",
]);

requireMarkers("apps/next-admin/playwright.rust-hosts.config.ts", [
  'testDir: "./tests/rust-hosts"',
  'baseURL: process.env.RUSTOK_BROWSER_BASE_URL || "http://127.0.0.1:5150"',
  '"x-tenant-slug": process.env.RUSTOK_BROWSER_TENANT_SLUG || "default"',
  'name: "rust-hosted-chromium"',
  'trace: "retain-on-failure"',
  'video: "retain-on-failure"',
]);

requireMarkers("apps/next-admin/tests/rust-hosts/embedded-hosts.spec.ts", [
  "health endpoint reports the running monolith",
  "server-hosted storefront renders under strict CSP",
  "embedded admin assets hydrate from the admin mount under strict CSP",
  "style-src-attr 'none'",
  "script-src-attr 'none'",
  "'unsafe-eval'",
  "style-src-attr 'unsafe-inline'",
  "<title>RusToK Storefront</title>",
  "<title>RusToK Admin</title>",
  "expectNonceBackedElements(html, \"script\", 1)",
  "expectNonceBackedElements(html, \"style\", 0)",
  "requestfailed",
  "pageerror",
]);

requireMarkers("scripts/build/build-embedded-admin.sh", [
  "--public-url",
  'TRUNK_BUILD_PUBLIC_URL="$public_url"',
  "cargo install trunk --version 0.21.14 --locked",
]);
requireMarkers("apps/server/src/services/app_router.rs", [
  'router.nest("/admin", admin_router)',
  "router.merge(storefront_router)",
  "nonce_trusted_admin_elements",
]);

requireMarkers(".github/workflows/hardening-gates.yml", [
  "Verify Rust-host browser smoke structure",
  "verify-rust-host-browser-contract.mjs",
]);
requireMarkers("scripts/verify/verify-all.sh", [
  "rust-host-browser-contract  Verify embedded Rust UI browser smoke structure",
  "verify-rust-host-browser-contract.mjs:Rust-host Browser Smoke Structure",
]);

if (failures.length > 0) {
  console.error("Rust-host browser smoke contract verification failed:");
  failures.forEach((failure) => console.error(`✗ ${failure}`));
  process.exit(Math.min(failures.length, 255));
}

console.log(
  "✔ commit-pinned Next and migration-prepared Rust-host browser evidence are structurally bound with digest-pinned PostgreSQL, bounded role and strict CSP assertions",
);

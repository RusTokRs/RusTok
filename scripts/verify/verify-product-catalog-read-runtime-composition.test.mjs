#!/usr/bin/env node

import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";

const scriptPath = path.resolve(
  "scripts/verify/verify-product-catalog-read-runtime-composition.mjs",
);

function write(root, relativePath, content) {
  const filePath = path.join(root, relativePath);
  mkdirSync(path.dirname(filePath), { recursive: true });
  writeFileSync(filePath, content);
}

function fixture(options = {}) {
  const root = mkdtempSync(path.join(tmpdir(), "rustok-product-read-runtime-"));
  write(
    root,
    "crates/rustok-product/src/runtime.rs",
    options.omitExternal
      ? "pub enum ProductCatalogReadProfile { EmbeddedNative } pub struct ProductCatalogReadRuntime pub fn in_process pub fn read_port pub const fn profile"
      : "pub enum ProductCatalogReadProfile { EmbeddedNative, External } pub struct ProductCatalogReadRuntime pub fn in_process pub fn external pub fn read_port pub const fn profile",
  );
  write(
    root,
    "crates/rustok-product/src/lib.rs",
    "mod runtime; ProductCatalogReadProfile ProductCatalogReadRuntime",
  );
  const directMarketplace = options.directMarketplace
    ? "let product_reader: Arc<dyn rustok_product::ProductCatalogReadPort> = Arc::new("
    : "";
  const directAi = options.directAi
    ? "rustok_product::CatalogService::new(server.db_clone(), event_bus)"
    : "";
  write(
    root,
    "apps/server/src/services/commerce_provider_runtime.rs",
    `host.shared_get::<rustok_product::ProductCatalogReadRuntime>() server.shared_get::<rustok_product::ProductCatalogReadRuntime>() rustok_product::ProductCatalogReadRuntime::in_process ProductCatalogReadRuntime must be initialized before marketplace listing SharedAiProductCatalogReadPort(runtime.read_port()) preserves_host_selected_external_product_catalog_runtime ProductCatalogReadProfile::External ${directMarketplace} ${directAi}`,
  );
  write(
    root,
    "crates/rustok-product/contracts/product-fba-registry.json",
    options.omitRegistry
      ? "{}"
      : `{"runtime_composition":{"runtime":"ProductCatalogReadRuntime","profiles":["embedded_native","external"],"source_complete_consumers":["ai-product","marketplace-listing"],"pending_consumers":["commerce-checkout-http","commerce-checkout-graphql","order-storefront-native"],"status":"source_complete_consumer_cutover_partial"}}`,
  );
  write(
    root,
    "crates/rustok-product/docs/implementation-plan.md",
    options.omitPlan
      ? "Product plan"
      : "ProductCatalogReadRuntime AI and Marketplace Listing checkout transport cutover remains open verify-product-catalog-read-runtime-composition.mjs",
  );
  return root;
}

function run(root) {
  return spawnSync("node", [scriptPath], {
    cwd: path.resolve("."),
    env: { ...process.env, RUSTOK_VERIFY_REPO_ROOT: root },
    encoding: "utf8",
  });
}

function reject(options, pattern) {
  const root = fixture(options);
  try {
    const result = run(root);
    assert.notEqual(result.status, 0, "expected runtime-composition mutation to fail");
    assert.match(result.stderr, pattern);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
}

test("runtime composition guard accepts canonical source fixture", () => {
  const root = fixture();
  try {
    const result = run(root);
    assert.equal(result.status, 0, result.stderr || result.stdout);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("runtime composition guard rejects missing external profile", () => {
  reject({ omitExternal: true }, /Product owner runtime/);
});

test("runtime composition guard rejects parallel Marketplace provider", () => {
  reject({ directMarketplace: true }, /parallel Marketplace Product provider/);
});

test("runtime composition guard rejects parallel AI provider", () => {
  reject({ directAi: true }, /parallel AI Product provider/);
});

test("runtime composition guard rejects missing registry evidence", () => {
  reject({ omitRegistry: true }, /Product FBA registry/);
});

test("runtime composition guard rejects missing plan handoff", () => {
  reject({ omitPlan: true }, /Product implementation plan/);
});

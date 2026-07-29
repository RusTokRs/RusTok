#!/usr/bin/env node

import { existsSync, readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = process.env.RUSTOK_VERIFY_REPO_ROOT
  ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT)
  : path.resolve(scriptDir, "../..");
const failures = [];

function read(relativePath) {
  const absolutePath = path.join(repoRoot, relativePath);
  if (!existsSync(absolutePath)) {
    failures.push(`${relativePath}: required runtime-composition file is missing`);
    return "";
  }
  return readFileSync(absolutePath, "utf8");
}

function requireAll(source, markers, description) {
  for (const marker of markers) {
    if (!source.includes(marker)) failures.push(`${description}: missing ${marker}`);
  }
}

function forbid(source, marker, description) {
  if (source.includes(marker)) failures.push(`${description}: forbidden ${marker}`);
}

const runtimePath = "crates/rustok-product/src/runtime.rs";
const libPath = "crates/rustok-product/src/lib.rs";
const hostPath = "apps/server/src/services/commerce_provider_runtime.rs";
const registryPath = "crates/rustok-product/contracts/product-fba-registry.json";
const planPath = "crates/rustok-product/docs/implementation-plan.md";
const runtime = read(runtimePath);
const lib = read(libPath);
const host = read(hostPath);
const registry = read(registryPath);
const plan = read(planPath);

requireAll(runtime, [
  "pub enum ProductCatalogReadProfile",
  "EmbeddedNative",
  "External",
  "pub struct ProductCatalogReadRuntime",
  "pub fn in_process",
  "pub fn external",
  "pub fn read_port",
  "pub const fn profile",
], "Product owner runtime");
requireAll(lib, [
  "mod runtime;",
  "ProductCatalogReadProfile",
  "ProductCatalogReadRuntime",
], "Product public exports");
requireAll(host, [
  "host.shared_get::<rustok_product::ProductCatalogReadRuntime>()",
  "server.shared_get::<rustok_product::ProductCatalogReadRuntime>()",
  "rustok_product::ProductCatalogReadRuntime::in_process",
  "ProductCatalogReadRuntime must be initialized before marketplace listing",
  "SharedAiProductCatalogReadPort(runtime.read_port())",
  "preserves_host_selected_external_product_catalog_runtime",
  "ProductCatalogReadProfile::External",
], "host composition");
forbid(
  host,
  "let product_reader: Arc<dyn rustok_product::ProductCatalogReadPort> = Arc::new(",
  "host composition must not construct a parallel Marketplace Product provider",
);
forbid(
  host,
  "rustok_product::CatalogService::new(server.db_clone(), event_bus)",
  "host composition must not construct a parallel AI Product provider",
);
requireAll(registry, [
  '"runtime_composition"',
  '"runtime": "ProductCatalogReadRuntime"',
  '"embedded_native"',
  '"external"',
  '"ai-product"',
  '"marketplace-listing"',
  '"commerce-checkout-http"',
  '"commerce-checkout-graphql"',
  '"order-storefront-native"',
  '"status": "source_complete_consumer_cutover_partial"',
], "Product FBA registry");
requireAll(plan, [
  "ProductCatalogReadRuntime",
  "AI and Marketplace Listing",
  "checkout transport cutover remains open",
  "verify-product-catalog-read-runtime-composition.mjs",
], "Product implementation plan");

if (failures.length > 0) {
  console.error("product catalog read runtime composition verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("product catalog read runtime composition verification passed");

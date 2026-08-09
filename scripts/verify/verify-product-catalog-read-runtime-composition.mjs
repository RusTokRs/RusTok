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
const httpPortPath = "crates/rustok-product/src/storefront_http_read_port.rs";
const tagPortPath = "crates/rustok-product/src/storefront_tag_read_port.rs";
const hostPath = "apps/server/src/services/commerce_provider_runtime.rs";
const registryPath = "crates/rustok-product/contracts/product-fba-registry.json";
const planPath = "crates/rustok-product/docs/implementation-plan.md";
const runtime = read(runtimePath);
const lib = read(libPath);
const httpPort = read(httpPortPath);
const tagPort = read(tagPortPath);
const host = read(hostPath);
const registry = read(registryPath);
const plan = read(planPath);

requireAll(runtime, [
  "pub enum ProductCatalogReadProfile",
  "EmbeddedNative",
  "External",
  "pub struct ProductCatalogReadRuntime",
  "storefront_http_read_port: Option<Arc<dyn ProductStorefrontHttpReadPort>>",
  "storefront_tag_read_port: Option<Arc<dyn ProductStorefrontTagReadPort>>",
  "pub fn in_process",
  ".with_storefront_http_read_port(catalog.clone())",
  ".with_storefront_tag_read_port(catalog)",
  "pub fn external",
  "pub fn read_port",
  "pub fn with_storefront_http_read_port",
  "pub fn storefront_http_read_port",
  "pub fn storefront_tag_read_port",
  "pub const fn profile",
], "Product owner runtime");
requireAll(lib, [
  "mod runtime;",
  "mod storefront_http_read_port;",
  "mod storefront_tag_read_port;",
  "ProductCatalogReadProfile",
  "ProductCatalogReadRuntime",
  "ProductStorefrontHttpReadPort",
  "LegacyStorefrontHttpProductsRequest",
  "ProductStorefrontTagReadPort",
  "ProductStorefrontTagHydrationRequest",
], "Product public exports");
requireAll(httpPort, [
  "pub trait ProductStorefrontHttpReadPort",
  "impl ProductStorefrontHttpReadPort for CatalogService",
  "MAX_LEGACY_STOREFRONT_HTTP_PRODUCTS_PER_PAGE: u64 = 100",
  "context.require_policy(PortCallPolicy::read())?",
  "rustok_inventory::is_metadata_visible_for_public_channel(",
  ".load_product_tag_map(tenant_id, &products, locale, Some(fallback_locale))",
], "Product optional Storefront HTTP capability");
requireAll(tagPort, [
  "pub trait ProductStorefrontTagReadPort",
  "impl ProductStorefrontTagReadPort for CatalogService",
  "MAX_STOREFRONT_TAG_HYDRATION_PRODUCTS: usize = 48",
  ".load_product_tag_map(",
], "Product optional Storefront tag capability");
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
  '"pending_consumers": []',
  '"status": "source_complete_consumer_cutover_complete"',
], "Product FBA registry");
requireAll(plan, [
  "ProductCatalogReadRuntime",
  "AI,",
  "checkout consumer source cutover is complete",
  "Concrete external transport execution remains open",
  "verify-product-catalog-read-runtime-composition.mjs",
], "Product implementation plan");

const externalStart = runtime.indexOf("pub fn external(read_port: Arc<dyn ProductCatalogReadPort>)");
const withHttpStart = runtime.indexOf("pub fn with_storefront_http_read_port(");
const withTagStart = runtime.indexOf("pub fn with_storefront_tag_read_port(");
if (externalStart < 0 || withHttpStart <= externalStart) {
  failures.push("Product owner runtime: external/HTTP capability boundaries are missing");
} else if (runtime.slice(externalStart, withHttpStart).includes("with_storefront_http_read_port")) {
  failures.push("Product owner runtime: external profile must not silently install embedded HTTP list capability");
}
if (externalStart < 0 || withTagStart <= externalStart) {
  failures.push("Product owner runtime: external/tag capability boundaries are missing");
} else if (runtime.slice(externalStart, withTagStart).includes("with_storefront_tag_read_port")) {
  failures.push("Product owner runtime: external profile must not silently install embedded tag hydration");
}

if (failures.length > 0) {
  console.error("product catalog read runtime composition verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("product catalog read runtime composition verification passed");

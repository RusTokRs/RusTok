#!/usr/bin/env node
// RusTok product storefront FFA boundary guardrails.
// Fast source-level checks for the module-owned core/transport/ui split.

import { existsSync, readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = process.env.RUSTOK_VERIFY_REPO_ROOT
  ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT)
  : path.resolve(scriptDir, "../..");
const failures = [];

function repoPath(relativePath) {
  return path.join(repoRoot, relativePath);
}

function readRepo(relativePath) {
  return readFileSync(repoPath(relativePath), "utf8");
}

function fail(message) {
  failures.push(message);
}

function assertExists(relativePath, description) {
  if (!existsSync(repoPath(relativePath))) fail(description);
}

function assertContains(text, pattern, description) {
  const found = typeof pattern === "string" ? text.includes(pattern) : pattern.test(text);
  if (!found) fail(description);
}

function assertNotContains(text, pattern, description) {
  const found = typeof pattern === "string" ? text.includes(pattern) : pattern.test(text);
  if (found) fail(description);
}

const libPath = "crates/rustok-product/storefront/src/lib.rs";
const corePath = "crates/rustok-product/storefront/src/core.rs";
const uiPath = "crates/rustok-product/storefront/src/ui/leptos.rs";
const transportPath = "crates/rustok-product/storefront/src/transport/mod.rs";
const legacyApiPath = "crates/rustok-product/storefront/src/api.rs";
const graphqlAdapterPath = "crates/rustok-product/storefront/src/transport/graphql_adapter.rs";
const nativeServerAdapterPath = "crates/rustok-product/storefront/src/transport/native_server_adapter.rs";
const cargoPath = "crates/rustok-product/storefront/Cargo.toml";
const implementationPlanPath = "crates/rustok-product/docs/implementation-plan.md";
const registryPath = "docs/modules/registry.md";
const packagePath = "package.json";

for (const filePath of [
  libPath,
  corePath,
  uiPath,
  transportPath,
  graphqlAdapterPath,
  nativeServerAdapterPath,
  cargoPath,
  implementationPlanPath,
  registryPath,
  packagePath,
]) {
  assertExists(filePath, `${filePath}: expected product storefront FFA boundary file`);
}
if (existsSync(repoPath(legacyApiPath))) {
  fail(`${legacyApiPath}: product storefront legacy api.rs must stay removed; transport adapters own raw operations`);
}

const lib = readRepo(libPath);
const core = readRepo(corePath);
const ui = readRepo(uiPath);
const transport = readRepo(transportPath);
const graphqlAdapter = readRepo(graphqlAdapterPath);
const nativeServerAdapter = readRepo(nativeServerAdapterPath);
const cargo = readRepo(cargoPath);
const implementationPlan = readRepo(implementationPlanPath);
const registry = readRepo(registryPath);
const packageJson = readRepo(packagePath);

assertContains(lib, "mod core;", `${libPath}: crate root must wire core`);
assertContains(lib, "mod transport;", `${libPath}: crate root must wire transport facade`);
assertContains(lib, "mod ui;", `${libPath}: crate root must wire UI adapters`);
assertContains(lib, "pub use ui::leptos::ProductView;", `${libPath}: crate root must re-export ProductView`);
assertNotContains(lib, "mod api;", `${libPath}: crate root must not wire legacy api adapter`);

for (const marker of ["leptos::", "leptos_", "#[component]", "#[server", "Resource<", "web_sys::"]) {
  assertNotContains(core, marker, `${corePath}: core must stay Leptos/server-function free (${marker})`);
}
for (const marker of [
  "build_product_catalog_rail_labels",
  "build_catalog_rail_view_model",
  "build_shell_view_model",
  "build_transport_error_dom_evidence",
  "build_selected_product_empty_view_model",
  "build_selected_product_view_model",
  "build_fetch_request",
  "build_route_input",
  "resolve_route_segment",
  "metadata_items",
  "show_empty_state",
]) {
  assertContains(core, marker, `${corePath}: expected core-owned storefront helper ${marker}`);
}

assertContains(ui, "use crate::core::{", `${uiPath}: Leptos adapter must import core-owned helpers`);
assertContains(ui, "use crate::transport;", `${uiPath}: Leptos adapter must call the module-owned transport facade`);
assertContains(ui, "build_product_catalog_rail_labels", `${uiPath}: UI must consume core-owned catalog rail labels`);
assertContains(ui, "build_catalog_rail_view_model", `${uiPath}: UI must consume core-owned catalog rail view-model`);
for (const marker of [
  "crate::i18n::t",
  "ProductCatalogRailLabels {",
  "product.list.title",
  "Published products",
  "No published products are available yet.",
  "Independent label",
]) {
  assertNotContains(ui, marker, `${uiPath}: catalog rail copy/label policy must stay in core (${marker})`);
}
for (const marker of ['<span>"|"</span>', "view_model.product_type", "view_model.vendor", "view_model.published_at"]) {
  assertNotContains(ui, marker, `${uiPath}: selected-product metadata display policy must stay in core (${marker})`);
}
for (const marker of ["view_model.items.is_empty()"]) {
  assertNotContains(ui, marker, `${uiPath}: catalog rail empty-state policy must stay in core (${marker})`);
}
for (const marker of ['unwrap_or_else(|| "products".to_string())', "PRODUCT_STOREFRONT_DEFAULT_ROUTE_SEGMENT"]) {
  assertNotContains(ui, marker, `${uiPath}: storefront route segment fallback policy must stay in core (${marker})`);
}
for (const marker of ["crate::api", /(^|[^A-Za-z0-9_])api::/, "#[server", "ProductService", "PricingService"]) {
  assertNotContains(ui, marker, `${uiPath}: UI adapter must not call raw transport or services (${marker})`);
}

assertContains(transport, "fetch_products", `${transportPath}: transport facade must expose fetch_products`);
assertContains(transport, "mod graphql_adapter;", `${transportPath}: transport facade must wire GraphQL adapter`);
assertContains(transport, "mod native_server_adapter;", `${transportPath}: transport facade must wire native server adapter`);
assertNotContains(transport, "crate::api", `${transportPath}: transport facade must not import legacy api module`);
assertContains(graphqlAdapter, "fetch_storefront_products_graphql", `${graphqlAdapterPath}: GraphQL adapter must expose GraphQL request path`);
assertContains(nativeServerAdapter, "#[server", `${nativeServerAdapterPath}: native server adapter must keep native server-function endpoint`);
assertContains(nativeServerAdapter, "GraphqlRequest", `${nativeServerAdapterPath}: moved adapter must keep GraphQL fallback request contract until split further`);
assertContains(nativeServerAdapter, "expect_context::<HostRuntimeContext>()", `${nativeServerAdapterPath}: native server adapter must use host runtime context`);
assertContains(nativeServerAdapter, "shared_get::<TransactionalEventBus>()", `${nativeServerAdapterPath}: native server adapter must receive event bus through host runtime context`);
assertContains(nativeServerAdapter, "runtime_ctx.db_clone()", `${nativeServerAdapterPath}: native server adapter must receive DB through host runtime context`);
assertNotContains(nativeServerAdapter, "loco_rs", `${nativeServerAdapterPath}: native server adapter must not depend on Loco AppContext`);
assertNotContains(nativeServerAdapter, "rustok_outbox::loco", `${nativeServerAdapterPath}: native server adapter must not use the outbox Loco adapter`);
assertNotContains(cargo, "loco-rs", `${cargoPath}: product storefront package must not depend on Loco`);
assertNotContains(cargo, "loco-adapter", `${cargoPath}: product storefront package must not enable the outbox Loco adapter`);
assertContains(implementationPlan, "verify-product-storefront-boundary.mjs", `${implementationPlanPath}: local plan must mention the product storefront fast boundary guardrail`);
assertContains(registry, "verify-product-storefront-boundary.mjs", `${registryPath}: central readiness board must mention the product storefront fast boundary guardrail`);
assertContains(packageJson, "verify:product:storefront-boundary", `${packagePath}: package scripts must expose product storefront boundary verification`);
assertContains(packageJson, "test:verify:product:storefront-boundary", `${packagePath}: package scripts must expose product storefront boundary fixture tests`);
assertContains(packageJson, "npm run test:verify:product:storefront-boundary", `${packagePath}: aggregate FFA fixture coverage must include product storefront boundary tests`);

if (failures.length > 0) {
  console.error("product storefront boundary verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("product storefront boundary verification passed");

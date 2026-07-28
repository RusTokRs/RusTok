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
const catalogControlsPath = "crates/rustok-product/storefront/src/catalog_controls.rs";
const corePath = "crates/rustok-product/storefront/src/core.rs";
const uiPath = "crates/rustok-product/storefront/src/ui/leptos.rs";
const transportPath = "crates/rustok-product/storefront/src/transport/mod.rs";
const catalogListNativePath = "crates/rustok-product/storefront/src/transport/catalog_list_native.rs";
const legacyApiPath = "crates/rustok-product/storefront/src/api.rs";
const graphqlAdapterPath = "crates/rustok-product/storefront/src/transport/graphql_adapter.rs";
const nativeServerAdapterPath = "crates/rustok-product/storefront/src/transport/native_server_adapter.rs";
const catalogQueriesPath = "crates/rustok-product/src/services/catalog/queries.rs";
const cargoPath = "crates/rustok-product/storefront/Cargo.toml";
const implementationPlanPath = "crates/rustok-product/docs/implementation-plan.md";
const registryPath = "docs/modules/registry.md";
const packagePath = "package.json";

for (const filePath of [
  libPath,
  catalogControlsPath,
  corePath,
  uiPath,
  transportPath,
  catalogListNativePath,
  graphqlAdapterPath,
  nativeServerAdapterPath,
  catalogQueriesPath,
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
const catalogControls = readRepo(catalogControlsPath);
const core = readRepo(corePath);
const ui = readRepo(uiPath);
const transport = readRepo(transportPath);
const catalogListNative = readRepo(catalogListNativePath);
const graphqlAdapter = readRepo(graphqlAdapterPath);
const nativeServerAdapter = readRepo(nativeServerAdapterPath);
const catalogQueries = readRepo(catalogQueriesPath);
const cargo = readRepo(cargoPath);
const implementationPlan = readRepo(implementationPlanPath);
const registry = readRepo(registryPath);
const packageJson = readRepo(packagePath);

assertContains(lib, "mod catalog_controls;", `${libPath}: crate root must wire typed catalog controls`);
assertContains(lib, "mod core;", `${libPath}: crate root must wire core`);
assertContains(lib, "mod transport;", `${libPath}: crate root must wire transport facade`);
assertContains(lib, "mod ui;", `${libPath}: crate root must wire UI adapters`);
assertContains(lib, "pub use ui::leptos::ProductView;", `${libPath}: crate root must re-export ProductView`);
assertNotContains(lib, "mod api;", `${libPath}: crate root must not wire legacy api adapter`);

assertContains(catalogControls, "pub struct CatalogListInput", `${catalogControlsPath}: storefront controls must use a typed catalog input`);
assertContains(catalogControls, "pub search: Option<String>", `${catalogControlsPath}: typed catalog input must carry optional search`);
assertContains(catalogControls, "normalize_optional_ui_text", `${catalogControlsPath}: catalog search must normalize optional UI text`);
assertContains(catalogControls, "build_catalog_search_labels", `${catalogControlsPath}: catalog search copy must stay outside the Leptos adapter`);

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
assertContains(ui, "build_catalog_list_input", `${uiPath}: UI must build typed catalog control state`);
assertContains(ui, 'read_route_query_value(&route_context, "search")', `${uiPath}: UI must read the snake_case search query key`);
assertContains(ui, 'name="search"', `${uiPath}: UI must expose the search query control`);
assertContains(ui, "transport::fetch_products(request, controls)", `${uiPath}: UI must pass typed controls to the transport facade`);
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
assertContains(transport, "CatalogListInput", `${transportPath}: transport facade must accept typed catalog controls`);
assertContains(transport, "mod catalog_list_native;", `${transportPath}: transport facade must wire the owner-native catalog list path`);
assertContains(transport, "catalog_list_native::fetch_products", `${transportPath}: selected native path must execute the owner-native catalog list`);
assertContains(transport, "mod graphql_adapter;", `${transportPath}: transport facade must wire GraphQL adapter`);
assertContains(transport, "mod native_server_adapter;", `${transportPath}: transport facade must wire native server adapter`);
assertNotContains(transport, "crate::api", `${transportPath}: transport facade must not import legacy api module`);
assertContains(graphqlAdapter, "GraphqlRequest", `${graphqlAdapterPath}: GraphQL adapter must expose GraphQL request path`);
assertContains(graphqlAdapter, "search: controls.search", `${graphqlAdapterPath}: GraphQL storefront list must carry typed search state`);
assertContains(catalogListNative, 'endpoint = "product/storefront/catalog-list"', `${catalogListNativePath}: native catalog list must use an owner endpoint`);
assertContains(catalogListNative, "StorefrontProductListQuery { search }", `${catalogListNativePath}: native catalog list must map typed search into the owner query`);
assertContains(catalogListNative, ".list_published_products_with_query(", `${catalogListNativePath}: native catalog list must execute the owner service query`);
assertNotContains(catalogListNative, "GraphqlRequest", `${catalogListNativePath}: native catalog list must not execute GraphQL`);
assertContains(catalogQueries, "pub async fn list_published_products_with_query", `${catalogQueriesPath}: Product owner service must expose the typed list query`);
assertContains(catalogQueries, "product_title_search_condition", `${catalogQueriesPath}: Product owner service must execute title search server-side`);
assertContains(nativeServerAdapter, "#[server", `${nativeServerAdapterPath}: native server adapter must keep native server-function endpoint`);
assertNotContains(nativeServerAdapter, "GraphqlRequest", `${nativeServerAdapterPath}: native adapter must not execute the parallel GraphQL contract`);
assertContains(nativeServerAdapter, "expect_context::<HostRuntimeContext>()", `${nativeServerAdapterPath}: native server adapter must use host runtime context`);
assertContains(nativeServerAdapter, "shared_get::<TransactionalEventBus>()", `${nativeServerAdapterPath}: native server adapter must receive event bus through host runtime context`);
assertContains(nativeServerAdapter, "runtime_ctx.db_clone()", `${nativeServerAdapterPath}: native server adapter must receive DB through host runtime context`);
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

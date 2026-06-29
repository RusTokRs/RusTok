#!/usr/bin/env node
// RusTok product admin FFA boundary guardrails.
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

const libPath = "crates/rustok-product/admin/src/lib.rs";
const corePath = "crates/rustok-product/admin/src/core.rs";
const uiPath = "crates/rustok-product/admin/src/ui/leptos.rs";
const transportPath = "crates/rustok-product/admin/src/transport.rs";
const legacyApiPath = "crates/rustok-product/admin/src/api.rs";
const graphqlAdapterPath = "crates/rustok-product/admin/src/transport/graphql_adapter.rs";
const implementationPlanPath = "crates/rustok-product/docs/implementation-plan.md";
const registryPath = "docs/modules/registry.md";
const packagePath = "package.json";

for (const filePath of [
  libPath,
  corePath,
  uiPath,
  transportPath,
  graphqlAdapterPath,
  implementationPlanPath,
  registryPath,
  packagePath,
]) {
  assertExists(filePath, `${filePath}: expected product admin FFA boundary file`);
}
if (existsSync(repoPath(legacyApiPath))) {
  fail(`${legacyApiPath}: product admin legacy api.rs must stay removed; transport/graphql_adapter.rs owns GraphQL operations`);
}

const lib = readRepo(libPath);
const core = readRepo(corePath);
const ui = readRepo(uiPath);
const transport = readRepo(transportPath);
const graphqlAdapter = readRepo(graphqlAdapterPath);
const implementationPlan = readRepo(implementationPlanPath);
const registry = readRepo(registryPath);
const packageJson = readRepo(packagePath);

assertContains(lib, "mod core;", `${libPath}: crate root must wire core`);
assertContains(lib, "mod transport;", `${libPath}: crate root must wire transport facade`);
assertContains(lib, "mod ui;", `${libPath}: crate root must wire UI adapters`);
assertContains(lib, "pub use ui::leptos::ProductAdmin;", `${libPath}: crate root must re-export ProductAdmin`);
assertNotContains(lib, "mod api;", `${libPath}: crate root must not wire legacy api adapter`);

for (const marker of ["leptos::", "leptos_", "#[component]", "#[server", "LocalResource", "WriteSignal", "web_sys::"]) {
  assertNotContains(core, marker, `${corePath}: core must stay Leptos/server-function free (${marker})`);
}
for (const marker of [
  "ProductAdminSaveCommand",
  "ProductAdminEditorFormState",
  "ProductAdminStatusMutationResultViewModel",
  "ProductAdminDeleteResultViewModel",
  "ProductAdminSeoPanelCopy",
  "ProductAdminSummaryPanelCopy",
  "parse_product_admin_inventory_quantity_input",
  "ProductAdminOpenProductViewModel",
  "product_admin_pricing_preview_state_from_result",
  "ProductAdminRouteQueryIntent",
  "ProductAdminSelectedProductQueryState",
  "product_admin_selected_product_query_state",
  "ProductAdminProductsLoadViewModel",
  "product_admin_products_load_view_from_result",
  "ProductAdminShippingProfilesLoadViewModel",
  "product_admin_shipping_profiles_load_view_from_result",
  "show_shipping_profile",
]) {
  assertContains(core, marker, `${corePath}: expected core-owned FFA helper ${marker}`);
}

assertContains(ui, "use crate::core::{", `${uiPath}: Leptos adapter must import core-owned helpers`);
assertContains(ui, "use crate::transport;", `${uiPath}: Leptos adapter must call the module-owned transport facade`);
assertContains(ui, "build_product_admin_save_command", `${uiPath}: UI must use core-owned save command preparation`);
assertContains(ui, "ProductAdminOpenProductViewModel", `${uiPath}: UI must consume core-owned open-product outcomes`);
assertContains(ui, "product_admin_pricing_preview_state_from_result", `${uiPath}: UI must use core-owned pricing preview state mapping`);
assertContains(ui, "build_product_admin_summary_panel_copy", `${uiPath}: UI must consume core-owned selected-summary panel copy`);
assertContains(ui, "product_admin_selected_product_query_state", `${uiPath}: UI must use core-owned selected product query state`);
assertContains(ui, "product_admin_products_load_view_from_result", `${uiPath}: UI must use core-owned products load-result normalization`);
assertContains(ui, "product_admin_shipping_profiles_load_view_from_result", `${uiPath}: UI must use core-owned shipping-profiles load-result normalization`);
for (const marker of ["crate::api", /(^|[^A-Za-z0-9_])api::/, "#[server", "ProductService", "PricingService"] ) {
  assertNotContains(ui, marker, `${uiPath}: UI adapter must not call raw transport or services (${marker})`);
}
for (const marker of ["product.summary.title", "Selected product"]) {
  assertNotContains(ui, marker, `${uiPath}: selected-summary panel copy must stay in core (${marker})`);
}
for (const marker of ["item_shipping_profile_label.is_some", "item_shipping_profile_label.clone().unwrap_or_default"]) {
  assertNotContains(ui, marker, `${uiPath}: shipping-profile chip display policy must stay in core (${marker})`);
}
for (const marker of ["product_id.trim().is_empty()", "selected_product_query.get() {"]) {
  assertNotContains(ui, marker, `${uiPath}: selected product query normalization must stay in core (${marker})`);
}
for (const marker of ["list.items.is_empty()", "list.items.into_iter().map"] ) {
  assertNotContains(ui, marker, `${uiPath}: products load-result normalization must stay in core (${marker})`);
}
assertNotContains(ui, "match shipping_profiles.get()", `${uiPath}: shipping-profile consumers must share core-owned load-result normalization`);

for (const marker of [
  "fetch_bootstrap",
  "fetch_products",
  "fetch_product",
  "fetch_product_pricing",
  "fetch_shipping_profiles",
  "create_product",
  "update_product",
  "change_product_status",
  "delete_product",
]) {
  assertContains(transport, marker, `${transportPath}: transport facade must expose ${marker}`);
}
assertContains(transport, "mod graphql_adapter;", `${transportPath}: transport facade must wire GraphQL adapter`);
assertContains(transport, "graphql_adapter::fetch_products", `${transportPath}: transport facade must delegate through GraphQL adapter`);
assertNotContains(transport, "use crate::api", `${transportPath}: transport facade must not delegate to legacy api module`);
assertNotContains(transport, "#[server", `${transportPath}: server/native endpoints must not live in the product admin transport facade`);
assertContains(graphqlAdapter, "GraphqlRequest", `${graphqlAdapterPath}: product admin GraphQL adapter must keep the GraphQL transport contract`);

assertContains(implementationPlan, "verify-product-admin-boundary.mjs", `${implementationPlanPath}: local plan must mention the product fast boundary guardrail`);
assertContains(registry, "verify-product-admin-boundary.mjs", `${registryPath}: central readiness board must mention the product fast boundary guardrail`);
assertContains(packageJson, "verify:product:admin-boundary", `${packagePath}: package scripts must expose product admin boundary verification`);
assertContains(packageJson, "test:verify:product:admin-boundary", `${packagePath}: package scripts must expose product admin boundary fixture tests`);
assertContains(packageJson, "npm run test:verify:product:admin-boundary", `${packagePath}: aggregate FFA fixture coverage must include product admin boundary tests`);

if (failures.length > 0) {
  console.error("product admin boundary verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("product admin boundary verification passed");

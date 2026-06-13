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
const apiPath = "crates/rustok-product/admin/src/api.rs";
const implementationPlanPath = "crates/rustok-product/docs/implementation-plan.md";
const registryPath = "docs/modules/registry.md";

for (const filePath of [
  libPath,
  corePath,
  uiPath,
  transportPath,
  apiPath,
  implementationPlanPath,
  registryPath,
]) {
  assertExists(filePath, `${filePath}: expected product admin FFA boundary file`);
}

const lib = readRepo(libPath);
const core = readRepo(corePath);
const ui = readRepo(uiPath);
const transport = readRepo(transportPath);
const api = readRepo(apiPath);
const implementationPlan = readRepo(implementationPlanPath);
const registry = readRepo(registryPath);

assertContains(lib, "mod core;", `${libPath}: crate root must wire core`);
assertContains(lib, "mod transport;", `${libPath}: crate root must wire transport facade`);
assertContains(lib, "mod ui;", `${libPath}: crate root must wire UI adapters`);
assertContains(lib, "pub use ui::leptos::ProductAdmin;", `${libPath}: crate root must re-export ProductAdmin`);

for (const marker of ["leptos::", "leptos_", "#[component]", "#[server", "LocalResource", "WriteSignal", "web_sys::"]) {
  assertNotContains(core, marker, `${corePath}: core must stay Leptos/server-function free (${marker})`);
}
for (const marker of [
  "ProductAdminSaveCommand",
  "ProductAdminEditorFormState",
  "ProductAdminStatusMutationResultViewModel",
  "ProductAdminDeleteResultViewModel",
  "ProductAdminSeoPanelCopy",
  "parse_product_admin_inventory_quantity_input",
  "ProductAdminOpenProductViewModel",
  "product_admin_pricing_preview_state_from_result",
  "ProductAdminRouteQueryIntent",
]) {
  assertContains(core, marker, `${corePath}: expected core-owned FFA helper ${marker}`);
}

assertContains(ui, "use crate::core::{", `${uiPath}: Leptos adapter must import core-owned helpers`);
assertContains(ui, "use crate::transport;", `${uiPath}: Leptos adapter must call the module-owned transport facade`);
assertContains(ui, "build_product_admin_save_command", `${uiPath}: UI must use core-owned save command preparation`);
assertContains(ui, "ProductAdminOpenProductViewModel", `${uiPath}: UI must consume core-owned open-product outcomes`);
assertContains(ui, "product_admin_pricing_preview_state_from_result", `${uiPath}: UI must use core-owned pricing preview state mapping`);
for (const marker of ["crate::api", /(^|[^A-Za-z0-9_])api::/, "#[server", "ProductService", "PricingService"] ) {
  assertNotContains(ui, marker, `${uiPath}: UI adapter must not call raw transport or services (${marker})`);
}

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
assertContains(transport, "use crate::api", `${transportPath}: transport facade may delegate to the current GraphQL/api adapter`);
assertNotContains(transport, "#[server", `${transportPath}: server/native endpoints must not live in the product admin transport facade`);
assertContains(api, "GraphqlRequest", `${apiPath}: product admin api adapter must keep the GraphQL transport contract`);

assertContains(implementationPlan, "verify-product-admin-boundary.mjs", `${implementationPlanPath}: local plan must mention the product fast boundary guardrail`);
assertContains(registry, "verify-product-admin-boundary.mjs", `${registryPath}: central readiness board must mention the product fast boundary guardrail`);

if (failures.length > 0) {
  console.error("product admin boundary verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("product admin boundary verification passed");

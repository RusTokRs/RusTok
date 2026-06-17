#!/usr/bin/env node
// RusTok pricing admin FFA boundary guardrails.
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

const libPath = "crates/rustok-pricing/admin/src/lib.rs";
const coreDirPath = "crates/rustok-pricing/admin/src/core";
const coreModPath = "crates/rustok-pricing/admin/src/core/mod.rs";
const corePresentationPath = "crates/rustok-pricing/admin/src/core/presentation.rs";
const coreRequestsPath = "crates/rustok-pricing/admin/src/core/requests.rs";
const coreRoutingPath = "crates/rustok-pricing/admin/src/core/routing.rs";
const uiPath = "crates/rustok-pricing/admin/src/ui/leptos.rs";
const transportPath = "crates/rustok-pricing/admin/src/transport.rs";
const apiPath = "crates/rustok-pricing/admin/src/api.rs";
const implementationPlanPath = "crates/rustok-pricing/docs/implementation-plan.md";
const registryPath = "docs/modules/registry.md";
const packagePath = "package.json";

for (const filePath of [
  libPath,
  coreModPath,
  corePresentationPath,
  coreRequestsPath,
  coreRoutingPath,
  uiPath,
  transportPath,
  apiPath,
  implementationPlanPath,
  registryPath,
  packagePath,
]) {
  assertExists(filePath, `${filePath}: expected pricing admin FFA boundary file`);
}
assertExists(coreDirPath, `${coreDirPath}: expected pricing admin core directory`);

const lib = readRepo(libPath);
const core = [coreModPath, corePresentationPath, coreRequestsPath, coreRoutingPath]
  .map((filePath) => readRepo(filePath))
  .join("\n");
const ui = readRepo(uiPath);
const transport = readRepo(transportPath);
const api = readRepo(apiPath);
const implementationPlan = readRepo(implementationPlanPath);
const registry = readRepo(registryPath);
const packageJson = readRepo(packagePath);

assertContains(lib, "mod api;", `${libPath}: crate root must keep the raw transport adapter private`);
assertContains(lib, "mod core;", `${libPath}: crate root must wire core`);
assertContains(lib, "mod transport;", `${libPath}: crate root must wire transport facade`);
assertContains(lib, "mod ui;", `${libPath}: crate root must wire UI adapters`);
assertContains(lib, "pub use ui::PricingAdmin;", `${libPath}: crate root must re-export PricingAdmin`);
for (const marker of [
  "pub async fn fetch_bootstrap",
  "pub async fn fetch_products",
  "pub async fn fetch_product",
  "pub async fn update_variant_price",
  "pub async fn preview_variant_discount",
  "pub async fn apply_variant_discount",
]) {
  assertNotContains(lib, marker, `${libPath}: crate root must not expose pre-FFA transport passthrough ${marker}`);
}

for (const marker of ["leptos::", "leptos_", "#[component]", "#[server", "LocalResource", "WriteSignal", "web_sys::", "GraphqlRequest"] ) {
  assertNotContains(core, marker, `${coreDirPath}: core must stay Leptos/server-function/GraphQL free (${marker})`);
}
for (const marker of [
  "PricingProductListItemViewModel",
  "PricingProductDetailHeaderViewModel",
  "PricingVariantCardViewModel",
  "PricingAdminRequestError",
  "build_price_draft",
  "build_discount_draft",
  "build_price_list_rule_draft",
  "selected_channel_key",
  "sanitize_resolution_context",
]) {
  assertContains(core, marker, `${coreDirPath}: expected core-owned FFA helper ${marker}`);
}

assertContains(ui, "use crate::core::{", `${uiPath}: Leptos adapter must import core-owned helpers`);
assertContains(ui, "use crate::transport;", `${uiPath}: Leptos adapter must call the module-owned transport facade`);
for (const marker of ["crate::api", /(^|[^A-Za-z0-9_])api::/, "#[server", "GraphqlRequest", "PricingService"] ) {
  assertNotContains(ui, marker, `${uiPath}: UI adapter must not call raw transport or services (${marker})`);
}

for (const marker of [
  "fetch_bootstrap",
  "fetch_active_price_lists",
  "fetch_products",
  "fetch_product",
  "update_variant_price",
  "preview_variant_discount",
  "apply_variant_discount",
  "update_price_list_rule",
  "update_price_list_scope",
]) {
  assertContains(transport, marker, `${transportPath}: transport facade must expose ${marker}`);
}
assertContains(transport, "use crate::api", `${transportPath}: transport facade may delegate to the current mixed native/GraphQL adapter`);
assertNotContains(transport, "#[server", `${transportPath}: server/native endpoints must not live in the pricing admin transport facade`);
assertContains(api, "GraphqlRequest", `${apiPath}: pricing admin api adapter must keep the GraphQL fallback contract`);
assertContains(api, "#[server", `${apiPath}: pricing admin api adapter must keep native server-function endpoints`);

assertContains(implementationPlan, "verify-pricing-admin-boundary.mjs", `${implementationPlanPath}: local plan must mention the pricing fast boundary guardrail`);
assertContains(registry, "verify-pricing-admin-boundary.mjs", `${registryPath}: central readiness board must mention the pricing fast boundary guardrail`);
assertContains(packageJson, "verify:pricing:admin-boundary", `${packagePath}: package scripts must expose the pricing admin boundary guardrail`);

if (failures.length > 0) {
  console.error("pricing admin boundary verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("pricing admin boundary verification passed");

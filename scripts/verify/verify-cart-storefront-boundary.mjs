#!/usr/bin/env node
// RusTok cart storefront FFA boundary guardrails.

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

const files = {
  lib: "crates/rustok-cart/storefront/src/lib.rs",
  coreDir: "crates/rustok-cart/storefront/src/core/mod.rs",
  ui: "crates/rustok-cart/storefront/src/ui/leptos.rs",
  transport: "crates/rustok-cart/storefront/src/transport/mod.rs",
  legacyApi: "crates/rustok-cart/storefront/src/api.rs",
  graphqlAdapter: "crates/rustok-cart/storefront/src/transport/graphql_adapter.rs",
  nativeServerAdapter: "crates/rustok-cart/storefront/src/transport/native_server_adapter.rs",
  implementationPlan: "crates/rustok-cart/docs/implementation-plan.md",
  registry: "docs/modules/registry.md",
  packageJson: "package.json",
};

for (const [name, filePath] of Object.entries(files)) {
  if (name === "legacyApi") continue;
  assertExists(filePath, `${filePath}: expected cart storefront FFA boundary file`);
}
if (existsSync(repoPath(files.legacyApi))) {
  fail(`${files.legacyApi}: cart storefront legacy api.rs must stay removed; transport adapters own raw operations`);
}

const lib = readRepo(files.lib);
const core = readRepo(files.coreDir);
const ui = readRepo(files.ui);
const transport = readRepo(files.transport);
const graphqlAdapter = readRepo(files.graphqlAdapter);
const nativeServerAdapter = readRepo(files.nativeServerAdapter);
const implementationPlan = readRepo(files.implementationPlan);
const registry = readRepo(files.registry);
const packageJson = readRepo(files.packageJson);

assertNotContains(lib, "mod api;", `${files.lib}: crate root must not wire legacy api adapter`);
assertContains(lib, "pub mod core;", `${files.lib}: crate root must expose core module`);
assertContains(lib, "mod transport;", `${files.lib}: crate root must wire transport facade`);
assertContains(lib, "mod ui;", `${files.lib}: crate root must wire UI adapters`);

for (const marker of ["leptos::", "leptos_", "#[component]", "#[server", "LocalResource", "WriteSignal", "web_sys::"]) {
  assertNotContains(core, marker, `${files.coreDir}: core must stay Leptos/server-function free (${marker})`);
}
for (const marker of ["CartFetchRequest", "CartLineItemDecrementRequest", "CartLineItemMutationRequest", "parse_cart_id", "parse_line_item_id"]) {
  assertContains(core, marker, `${files.coreDir}: expected core-owned cart helper ${marker}`);
}

assertContains(ui, "use crate::core", `${files.ui}: Leptos adapter must consume core layer`);
assertContains(ui, "use crate::transport", `${files.ui}: Leptos adapter must consume transport facade`);
for (const marker of ["crate::api", /(^|[^A-Za-z0-9_])api::/, "#[server", "CartService"]) {
  assertNotContains(ui, marker, `${files.ui}: UI adapter must not call raw transport or services (${marker})`);
}

for (const marker of ["fetch_cart", "decrement_line_item", "remove_line_item"]) {
  assertContains(transport, marker, `${files.transport}: transport facade must expose ${marker}`);
}
assertContains(transport, "mod graphql_adapter;", `${files.transport}: transport must wire GraphQL adapter`);
assertContains(transport, "mod native_server_adapter;", `${files.transport}: transport must wire native server adapter`);
assertNotContains(transport, "crate::api", `${files.transport}: transport facade must not import legacy api module`);
assertContains(graphqlAdapter, "fetch_storefront_cart_graphql", `${files.graphqlAdapter}: GraphQL adapter must delegate to GraphQL path`);
assertContains(nativeServerAdapter, "#[server", `${files.nativeServerAdapter}: native server adapter must keep server functions`);
assertContains(nativeServerAdapter, "GraphqlRequest", `${files.nativeServerAdapter}: moved adapter must keep GraphQL fallback request contract until split further`);
assertNotContains(nativeServerAdapter, "sellerScope } adjustments", `${files.nativeServerAdapter}: cart line-item read query must not request legacy sellerScope`);
assertNotContains(nativeServerAdapter, "sellerScope lineItemIds", `${files.nativeServerAdapter}: cart delivery-group read query must not request legacy sellerScope`);

assertContains(implementationPlan, "verify-cart-storefront-boundary.mjs", `${files.implementationPlan}: local plan must mention cart storefront guardrail`);
assertContains(registry, "verify-cart-storefront-boundary.mjs", `${files.registry}: central readiness board must mention cart storefront guardrail`);
assertContains(packageJson, "verify:cart:storefront-boundary", `${files.packageJson}: package scripts must expose cart storefront boundary verification`);
assertContains(packageJson, "test:verify:cart:storefront-boundary", `${files.packageJson}: package scripts must expose cart storefront fixture tests`);
assertContains(packageJson, "npm run test:verify:cart:storefront-boundary", `${files.packageJson}: aggregate FFA fixture tests must include cart storefront fixtures`);

if (failures.length > 0) {
  console.error("cart storefront boundary verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("cart storefront boundary verification passed");

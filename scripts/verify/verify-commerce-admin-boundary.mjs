#!/usr/bin/env node
// RusTok commerce admin FFA boundary guardrails.

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
  lib: "crates/rustok-commerce/admin/src/lib.rs",
  core: "crates/rustok-commerce/admin/src/core/mod.rs",
  ui: "crates/rustok-commerce/admin/src/ui/leptos.rs",
  transport: "crates/rustok-commerce/admin/src/transport/mod.rs",
  shippingProfile: "crates/rustok-commerce/admin/src/transport/shipping_profile.rs",
  promotion: "crates/rustok-commerce/admin/src/transport/promotion.rs",
  orderChange: "crates/rustok-commerce/admin/src/transport/order_change.rs",
  rawAdapter: "crates/rustok-commerce/admin/src/transport/raw_adapter.rs",
  commerceRoot: "crates/rustok-commerce/src/lib.rs",
  legacyApi: "crates/rustok-commerce/admin/src/api.rs",
  implementationPlan: "crates/rustok-commerce/docs/implementation-plan.md",
  registry: "docs/modules/registry.md",
  packageJson: "package.json",
};

for (const [name, filePath] of Object.entries(files)) {
  if (name === "legacyApi") continue;
  assertExists(filePath, `${filePath}: expected commerce admin FFA boundary file`);
}
if (existsSync(repoPath(files.legacyApi))) {
  fail(`${files.legacyApi}: commerce admin legacy api.rs must stay removed; transport/raw_adapter.rs owns raw operations`);
}

const lib = readRepo(files.lib);
const core = readRepo(files.core);
const ui = readRepo(files.ui);
const transport = readRepo(files.transport);
const shippingProfile = readRepo(files.shippingProfile);
const promotion = readRepo(files.promotion);
const orderChange = readRepo(files.orderChange);
const rawAdapter = readRepo(files.rawAdapter);
const commerceRoot = readRepo(files.commerceRoot);
const implementationPlan = readRepo(files.implementationPlan);
const registry = readRepo(files.registry);
const packageJson = readRepo(files.packageJson);

assertNotContains(lib, "mod api;", `${files.lib}: crate root must not wire legacy api module`);
assertContains(lib, "mod core;", `${files.lib}: crate root must wire core`);
assertContains(lib, "mod transport;", `${files.lib}: crate root must wire transport`);
assertContains(lib, "mod ui;", `${files.lib}: crate root must wire UI`);

for (const marker of ["leptos::", "leptos_", "#[component]", "#[server", "LocalResource", "WriteSignal", "web_sys::"]) {
  assertNotContains(core, marker, `${files.core}: core must stay Leptos/server-function free (${marker})`);
}
assertContains(ui, /use crate::(?:\{[^}]*transport[^}]*\}|transport)/, `${files.ui}: UI must consume transport facade`);
for (const marker of ["crate::api", /(^|[^A-Za-z0-9_])api::/, "#[server"]) {
  assertNotContains(ui, marker, `${files.ui}: UI adapter must not call raw transport or server functions (${marker})`);
}

assertContains(transport, "mod raw_adapter;", `${files.transport}: transport must wire raw adapter inside transport boundary`);
for (const [source, filePath] of [
  [shippingProfile, files.shippingProfile],
  [promotion, files.promotion],
  [orderChange, files.orderChange],
]) {
  assertContains(source, "use super::raw_adapter", `${filePath}: transport slice must delegate through raw_adapter`);
  assertNotContains(source, "crate::api", `${filePath}: transport slice must not import legacy api module`);
}
assertContains(rawAdapter, "pub enum ApiError", `${files.rawAdapter}: raw adapter must own shared ApiError envelope`);
assertContains(rawAdapter, "GraphqlRequest", `${files.rawAdapter}: raw adapter must keep GraphQL request contract until split further`);
assertContains(rawAdapter, "#[server", `${files.rawAdapter}: raw adapter must keep native server-function endpoints`);

for (const marker of [
  "pub use graphql::{CommerceMutation, CommerceQuery}",
  "pub use state_machine::{",
]) {
  assertNotContains(commerceRoot, marker, `${files.commerceRoot}: root GraphQL/state-machine aliases must stay removed (${marker})`);
}
assertContains(commerceRoot, "pub mod graphql;", `${files.commerceRoot}: GraphQL module path must remain explicit`);
assertContains(commerceRoot, "pub mod state_machine;", `${files.commerceRoot}: state-machine module path must remain explicit`);

assertContains(implementationPlan, "verify-commerce-admin-boundary.mjs", `${files.implementationPlan}: local plan must mention commerce admin guardrail`);
assertContains(implementationPlan, "admin/src/transport/raw_adapter.rs", `${files.implementationPlan}: local plan must document commerce admin raw adapter location`);
assertContains(implementationPlan, "root GraphQL and state-machine aliases", `${files.implementationPlan}: local plan must document removed root GraphQL/state-machine aliases`);
assertContains(registry, "verify-commerce-admin-boundary.mjs", `${files.registry}: central readiness board must mention commerce admin guardrail`);
assertContains(registry, "root GraphQL/state-machine aliases", `${files.registry}: central registry must document removed root GraphQL/state-machine aliases`);
assertContains(packageJson, "verify:commerce:admin-boundary", `${files.packageJson}: package scripts must expose commerce admin boundary verification`);
assertContains(packageJson, "test:verify:commerce:admin-boundary", `${files.packageJson}: package scripts must expose commerce admin fixture tests`);
assertContains(packageJson, "npm run verify:commerce:admin-boundary", `${files.packageJson}: aggregate FFA verification must include commerce admin guardrail`);
assertContains(packageJson, "npm run test:verify:commerce:admin-boundary", `${files.packageJson}: aggregate FFA fixture tests must include commerce admin fixtures`);

if (failures.length > 0) {
  console.error("commerce admin boundary verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("commerce admin boundary verification passed");

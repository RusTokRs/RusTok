#!/usr/bin/env node
// RusTok fulfillment admin FFA boundary guardrails.
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

const libPath = "crates/rustok-fulfillment/admin/src/lib.rs";
const corePath = "crates/rustok-fulfillment/admin/src/core.rs";
const uiPath = "crates/rustok-fulfillment/admin/src/ui/leptos.rs";
const transportPath = "crates/rustok-fulfillment/admin/src/transport.rs";
const legacyApiPath = "crates/rustok-fulfillment/admin/src/api.rs";
const graphqlAdapterPath = "crates/rustok-fulfillment/admin/src/transport/graphql_adapter.rs";
const implementationPlanPath = "crates/rustok-fulfillment/docs/implementation-plan.md";
const registryPath = "docs/modules/registry.md";

for (const filePath of [
  libPath,
  corePath,
  uiPath,
  transportPath,
  graphqlAdapterPath,
  implementationPlanPath,
  registryPath,
]) {
  assertExists(filePath, `${filePath}: expected fulfillment admin FFA boundary file`);
}
if (existsSync(repoPath(legacyApiPath))) {
  fail(`${legacyApiPath}: fulfillment admin legacy api.rs must stay removed; transport/graphql_adapter.rs owns GraphQL operations`);
}

const lib = readRepo(libPath);
const core = readRepo(corePath);
const ui = readRepo(uiPath);
const transport = readRepo(transportPath);
const graphqlAdapter = readRepo(graphqlAdapterPath);
const implementationPlan = readRepo(implementationPlanPath);
const registry = readRepo(registryPath);

assertNotContains(lib, "mod api;", `${libPath}: crate root must not wire legacy api adapter`);
assertContains(lib, "mod core;", `${libPath}: crate root must wire core`);
assertContains(lib, "mod transport;", `${libPath}: crate root must wire transport facade`);
assertContains(lib, "mod ui;", `${libPath}: crate root must wire UI adapters`);
assertContains(lib, "pub use ui::FulfillmentAdmin;", `${libPath}: crate root must re-export FulfillmentAdmin`);
for (const marker of [/pub async fn fetch_/, /pub async fn create_/, /pub async fn update_/, /pub async fn deactivate_/, /pub async fn reactivate_/]) {
  assertNotContains(lib, marker, `${libPath}: crate root must not expose public transport passthroughs (${marker})`);
}

for (const marker of ["leptos::", "leptos_", "#[component]", "#[server", "LocalResource", "WriteSignal", "web_sys::"]) {
  assertNotContains(core, marker, `${corePath}: core must stay Leptos/server-function free (${marker})`);
}
for (const marker of [
  "ShippingOptionListRequest",
  "ShippingProfileListRequest",
  "shipping_option_list_request",
  "shipping_profile_list_request",
  "text_or_none",
]) {
  assertContains(core, marker, `${corePath}: expected core-owned FFA helper ${marker}`);
}

assertContains(ui, "use crate::core::{shipping_option_list_request, shipping_profile_list_request};", `${uiPath}: UI must consume core-owned request helpers`);
assertContains(ui, "use crate::transport;", `${uiPath}: UI must call the module-owned transport facade`);
for (const marker of [
  "transport::fetch_bootstrap",
  "transport::fetch_shipping_options",
  "transport::fetch_shipping_profiles",
  "transport::fetch_shipping_option",
  "transport::create_shipping_option",
  "transport::update_shipping_option",
  "transport::deactivate_shipping_option",
  "transport::reactivate_shipping_option",
]) {
  assertContains(ui, marker, `${uiPath}: expected UI call through transport facade ${marker}`);
}
for (const marker of ["crate::api", /(^|[^A-Za-z0-9_])api::/, "#[server", "FulfillmentService", "ShippingOptionService"]) {
  assertNotContains(ui, marker, `${uiPath}: UI adapter must not call raw transport or services (${marker})`);
}

for (const marker of [
  "fetch_bootstrap",
  "fetch_shipping_options",
  "fetch_shipping_option",
  "fetch_shipping_profiles",
  "create_shipping_option",
  "update_shipping_option",
  "deactivate_shipping_option",
  "reactivate_shipping_option",
]) {
  assertContains(transport, marker, `${transportPath}: transport facade must expose ${marker}`);
}
assertContains(transport, "mod graphql_adapter;", `${transportPath}: transport facade must wire GraphQL adapter`);
assertContains(transport, "graphql_adapter::fetch_bootstrap", `${transportPath}: transport facade must delegate through GraphQL adapter`);
assertNotContains(transport, "use crate::api", `${transportPath}: transport facade must not delegate to legacy api module`);
assertNotContains(transport, "#[server", `${transportPath}: server/native endpoints must not live in the fulfillment admin transport facade`);
assertContains(graphqlAdapter, "GraphqlRequest", `${graphqlAdapterPath}: fulfillment admin GraphQL adapter must keep the GraphQL transport contract`);

assertContains(implementationPlan, "verify-fulfillment-admin-boundary.mjs", `${implementationPlanPath}: local plan must mention the fulfillment fast boundary guardrail`);
assertContains(registry, "verify-fulfillment-admin-boundary.mjs", `${registryPath}: central readiness board must mention the fulfillment fast boundary guardrail`);

if (failures.length > 0) {
  console.error("fulfillment admin boundary verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("fulfillment admin boundary verification passed");

#!/usr/bin/env node
// RusTok customer admin FFA boundary guardrails.
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

function assertMissing(relativePath, description) {
  if (existsSync(repoPath(relativePath))) fail(description);
}

function assertContains(text, pattern, description) {
  const found = typeof pattern === "string" ? text.includes(pattern) : pattern.test(text);
  if (!found) fail(description);
}

function assertNotContains(text, pattern, description) {
  const found = typeof pattern === "string" ? text.includes(pattern) : pattern.test(text);
  if (found) fail(description);
}

const libPath = "crates/rustok-customer/admin/src/lib.rs";
const corePath = "crates/rustok-customer/admin/src/core.rs";
const transportModPath = "crates/rustok-customer/admin/src/transport/mod.rs";
const nativeAdapterPath = "crates/rustok-customer/admin/src/transport/native_server_adapter.rs";
const uiPath = "crates/rustok-customer/admin/src/ui/leptos.rs";
const cargoPath = "crates/rustok-customer/admin/Cargo.toml";
const readmePath = "crates/rustok-customer/admin/README.md";
const localPlanPath = "crates/rustok-customer/docs/implementation-plan.md";
const registryPath = "docs/modules/registry.md";

for (const filePath of [
  libPath,
  corePath,
  transportModPath,
  nativeAdapterPath,
  uiPath,
  cargoPath,
  readmePath,
  localPlanPath,
  registryPath,
]) {
  assertExists(filePath, `${filePath}: expected customer admin boundary file`);
}
assertMissing(
  "crates/rustok-customer/admin/src/api.rs",
  "crates/rustok-customer/admin/src/api.rs: pre-FFA api facade must stay removed",
);

const lib = readRepo(libPath);
const core = readRepo(corePath);
const transportMod = readRepo(transportModPath);
const nativeAdapter = readRepo(nativeAdapterPath);
const ui = readRepo(uiPath);
const cargoToml = readRepo(cargoPath);
const readme = readRepo(readmePath);
const localPlan = readRepo(localPlanPath);
const registry = readRepo(registryPath);

assertContains(lib, "mod core;", `${libPath}: crate root must wire core`);
assertContains(lib, "mod transport;", `${libPath}: crate root must wire transport facade`);
assertContains(lib, "mod ui;", `${libPath}: crate root must wire UI adapters`);
assertContains(lib, "pub use ui::CustomerAdmin;", `${libPath}: crate root must re-export the Leptos adapter surface`);
assertNotContains(lib, "mod api;", `${libPath}: crate root must not wire the pre-FFA api facade`);

for (const marker of ["leptos::", "leptos_", "#[component]", "#[server]", "LocalResource"]) {
  assertNotContains(core, marker, `${corePath}: core must stay framework/transport-runtime free (${marker})`);
}
for (const marker of [
  "CustomerAdminDraftInput",
  "CustomerAdminSubmitCommand",
  "CustomerAdminSubmitCommandError",
  "build_customer_admin_submit_command",
]) {
  assertContains(core, marker, `${corePath}: core must own ${marker}`);
}

assertContains(transportMod, "mod native_server_adapter;", `${transportModPath}: transport facade must wire native adapter`);
assertContains(transportMod, "native::fetch_customers(search, page, per_page).await", `${transportModPath}: facade must call native customer list path`);
assertContains(transportMod, "native::create_customer(payload).await", `${transportModPath}: facade must call native customer create path`);
assertContains(transportMod, "native::update_customer(customer_id, payload).await", `${transportModPath}: facade must call native customer update path`);
assertNotContains(transportMod, "#[server", `${transportModPath}: server-function endpoints belong in native_server_adapter.rs`);

for (const endpoint of [
  "customer/bootstrap",
  "customer/list",
  "customer/detail",
  "customer/create",
  "customer/update",
]) {
  assertContains(nativeAdapter, endpoint, `${nativeAdapterPath}: native adapter must own ${endpoint} endpoint`);
}
assertContains(nativeAdapter, "HostRuntimeContext", `${nativeAdapterPath}: native adapter must consume neutral host runtime context`);
assertContains(nativeAdapter, "runtime_ctx.db_clone()", `${nativeAdapterPath}: native adapter must build services from the neutral DB handle`);

assertContains(ui, "use crate::core::{", `${uiPath}: Leptos adapter must consume core helpers`);
assertContains(ui, "use crate::i18n::t;", `${uiPath}: Leptos adapter must consume package i18n facade`);
assertContains(ui, "use crate::transport;", `${uiPath}: Leptos adapter must consume transport facade`);
assertContains(ui, "transport::fetch_customers", `${uiPath}: Leptos adapter must call module-owned transport facade`);
assertNotContains(ui, "native_server_adapter::", `${uiPath}: UI adapter must not call raw native adapter`);

assertContains(readme, "HostRuntimeContext", `${readmePath}: README must record host-neutral native admin transport`);
assertContains(localPlan, "verify-customer-admin-boundary.mjs", `${localPlanPath}: local plan must record fast boundary guardrail evidence`);
assertContains(registry, "verify-customer-admin-boundary.mjs", `${registryPath}: central registry must record customer admin boundary guardrail`);

if (failures.length > 0) {
  console.error("Customer admin boundary verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Customer admin boundary verification passed");

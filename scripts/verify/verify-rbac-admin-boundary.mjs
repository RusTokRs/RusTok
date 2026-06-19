#!/usr/bin/env node
// RusTok RBAC admin FFA boundary guardrails.
// Fast source-level checks for the module-owned core/transport/ui split and
// the documented native-only RBAC admin overview transport exception.

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

const libPath = "crates/rustok-rbac/admin/src/lib.rs";
const corePath = "crates/rustok-rbac/admin/src/core.rs";
const uiPath = "crates/rustok-rbac/admin/src/ui/leptos.rs";
const transportModPath = "crates/rustok-rbac/admin/src/transport/mod.rs";
const nativeAdapterPath = "crates/rustok-rbac/admin/src/transport/native_server_adapter.rs";
const localPlanPath = "crates/rustok-rbac/docs/implementation-plan.md";
const registryPath = "docs/modules/registry.md";

for (const filePath of [
  libPath,
  corePath,
  uiPath,
  transportModPath,
  nativeAdapterPath,
  localPlanPath,
  registryPath,
]) {
  assertExists(filePath, `${filePath}: expected RBAC admin FFA boundary file`);
}
assertMissing(
  "crates/rustok-rbac/admin/src/api.rs",
  "crates/rustok-rbac/admin/src/api.rs: pre-FFA api facade must stay removed",
);

const lib = readRepo(libPath);
const core = readRepo(corePath);
const ui = readRepo(uiPath);
const transportMod = readRepo(transportModPath);
const nativeAdapter = readRepo(nativeAdapterPath);
const localPlan = readRepo(localPlanPath);
const registry = readRepo(registryPath);

assertContains(lib, "mod core;", `${libPath}: crate root must wire core`);
assertContains(lib, "mod transport;", `${libPath}: crate root must wire transport facade`);
assertContains(lib, "mod ui;", `${libPath}: crate root must wire UI adapters`);
assertContains(lib, "pub use ui::leptos::RbacAdmin;", `${libPath}: crate root must re-export the Leptos adapter surface`);
assertNotContains(lib, "mod api;", `${libPath}: crate root must not wire the pre-FFA api facade`);

for (const marker of ["leptos::", "leptos_", "#[component]", "#[server]", "LocalResource"]) {
  assertNotContains(core, marker, `${corePath}: core must stay Leptos/server-function free (${marker})`);
}
assertContains(core, "RbacAdminOverviewViewModel", `${corePath}: core must own overview view-model policy`);
assertContains(core, "build_rbac_admin_overview_view_model", `${corePath}: core must own overview mapping`);
assertContains(core, "format_rbac_admin_bootstrap_error", `${corePath}: core must own bootstrap error formatting`);
assertContains(core, "overview_view_model_formats_bootstrap_without_framework_runtime", `${corePath}: core view-model must have framework-free unit coverage`);

assertContains(ui, "use crate::core::{build_rbac_admin_overview_view_model, format_rbac_admin_bootstrap_error};", `${uiPath}: Leptos adapter must consume core helpers`);
assertContains(ui, "use crate::transport;", `${uiPath}: Leptos adapter must consume transport facade`);
assertContains(ui, "transport::fetch_bootstrap", `${uiPath}: Leptos adapter must call module-owned transport facade`);
for (const marker of ["mod api;", "crate::api", /(^|[^A-Za-z0-9_])api::/, "native_server_adapter::", "fetch_bootstrap_native", "#[server"]) {
  assertNotContains(ui, marker, `${uiPath}: UI adapter must not call raw/pre-FFA transport (${marker})`);
}

assertContains(transportMod, "mod native_server_adapter;", `${transportModPath}: transport facade must wire native adapter`);
assertContains(transportMod, "RbacAdminTransportError", `${transportModPath}: transport facade must expose typed error envelope`);
assertContains(transportMod, "fetch_bootstrap_native().await", `${transportModPath}: facade must call native bootstrap path`);
assertNotContains(transportMod, "#[server", `${transportModPath}: server-function endpoint belongs in native_server_adapter.rs`);
assertNotContains(transportMod, "graphql", `${transportModPath}: RBAC overview must not invent a package-local GraphQL fallback`);

assertContains(nativeAdapter, "#[server", `${nativeAdapterPath}: native adapter must contain server-function endpoint`);
assertContains(nativeAdapter, "fetch_bootstrap_native", `${nativeAdapterPath}: native adapter must own bootstrap server-function endpoint`);
assertContains(nativeAdapter, "ModuleRegistry", `${nativeAdapterPath}: native adapter must build module permission catalog from host registry`);
assertContains(nativeAdapter, "infer_user_role_from_permissions", `${nativeAdapterPath}: native adapter must derive role snapshot from auth permissions`);

assertContains(localPlan, "native-only", `${localPlanPath}: local plan must document native-only admin transport exception`);
assertContains(localPlan, "verify-rbac-admin-boundary.mjs", `${localPlanPath}: local plan must record fast boundary guardrail evidence`);
assertContains(registry, "verify-rbac-admin-boundary.mjs", `${registryPath}: central registry must record RBAC admin boundary guardrail`);

if (failures.length > 0) {
  console.error("RBAC admin boundary verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("RBAC admin boundary verification passed");

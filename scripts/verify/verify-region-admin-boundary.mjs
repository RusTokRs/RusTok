#!/usr/bin/env node
// RusTok region admin FFA boundary guardrails.
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

const libPath = "crates/rustok-region/admin/src/lib.rs";
const corePath = "crates/rustok-region/admin/src/core.rs";
const uiPath = "crates/rustok-region/admin/src/ui/leptos.rs";
const transportPath = "crates/rustok-region/admin/src/transport/mod.rs";
const apiPath = "crates/rustok-region/admin/src/api.rs";
const implementationPlanPath = "crates/rustok-region/docs/implementation-plan.md";
const registryPath = "docs/modules/registry.md";
const packagePath = "package.json";
const verifierTestPath = "scripts/verify/verify-region-admin-boundary.test.mjs";

for (const filePath of [
  libPath,
  corePath,
  uiPath,
  transportPath,
  apiPath,
  implementationPlanPath,
  registryPath,
  packagePath,
  verifierTestPath,
]) {
  assertExists(filePath, `${filePath}: expected region admin FFA boundary file`);
}

const lib = readRepo(libPath);
const core = readRepo(corePath);
const ui = readRepo(uiPath);
const transport = readRepo(transportPath);
const api = readRepo(apiPath);
const implementationPlan = readRepo(implementationPlanPath);
const registry = readRepo(registryPath);
const packageJson = readRepo(packagePath);
const verifierTest = readRepo(verifierTestPath);

assertContains(lib, "mod core;", `${libPath}: crate root must wire core`);
assertContains(lib, "mod transport;", `${libPath}: crate root must wire transport facade`);
assertContains(lib, "mod ui;", `${libPath}: crate root must wire UI adapters`);
assertContains(lib, "pub use ui::RegionAdmin;", `${libPath}: crate root must re-export the Leptos adapter surface`);

for (const marker of ["leptos::", "leptos_", "#[component]", "#[server", "LocalResource", "WriteSignal", "web_sys::"]) {
  assertNotContains(core, marker, `${corePath}: core must stay Leptos/server-function free (${marker})`);
}
for (const marker of [
  "RegionAdminSubmitInput",
  "RegionAdminSubmitCommand",
  "RegionAdminSubmitError",
  "RegionAdminSubmitErrorLabels",
  "RegionAdminTransportErrorLabels",
  "prepare_region_admin_submit",
  "region_admin_submit_error_message",
  "region_admin_save_region_error_message",
  "RegionAdminRouteQueryUpdate",
  "RegionAdminRouteQueryWrite",
  "region_admin_route_query_write",
  "optional_region_admin_route_query_write",
  "RegionAdminDetailPanelViewModel",
  "RegionAdminOpenDetailViewModel",
  "RegionAdminSaveSuccessViewModel",
  "region_admin_save_success",
  "region_admin_open_detail_success",
  "region_admin_open_detail_error",
]) {
  assertContains(core, marker, `${corePath}: expected core-owned FFA helper ${marker}`);
}

assertContains(ui, "use crate::core::{", `${uiPath}: Leptos adapter must import core-owned helpers`);
assertContains(ui, "prepare_region_admin_submit", `${uiPath}: Leptos adapter must call core-owned submit preparation`);
assertContains(ui, "RegionAdminSubmitError", `${uiPath}: Leptos adapter must consume typed submit errors`);
assertContains(ui, "crate::transport::create_region", `${uiPath}: Leptos adapter must call module-owned transport facade for create`);
assertContains(ui, "crate::transport::update_region", `${uiPath}: Leptos adapter must call module-owned transport facade for update`);
for (const marker of ["crate::api", /(^|[^A-Za-z0-9_])api::/, "#[server", "RegionService"] ) {
  assertNotContains(ui, marker, `${uiPath}: UI adapter must not call raw/native transport or service (${marker})`);
}

for (const marker of [
  "fetch_bootstrap",
  "fetch_regions",
  "fetch_region_detail",
  "create_region",
  "update_region",
]) {
  assertContains(transport, marker, `${transportPath}: transport facade must expose ${marker}`);
}
assertContains(transport, "use crate::api;", `${transportPath}: current temporary admin adapter may delegate to api facade`);
assertContains(api, "#[server", `${apiPath}: temporary native server-function adapter must keep native endpoints`);
assertContains(api, "RegionService", `${apiPath}: native adapter must own service calls, not the UI layer`);

assertContains(implementationPlan, "FFA slice #31", `${implementationPlanPath}: local plan must record slice #31`);
assertContains(implementationPlan, "verify-region-admin-boundary.mjs", `${implementationPlanPath}: local plan must mention the fast boundary guardrail`);
assertContains(registry, "slice #40", `${registryPath}: central readiness board must record slice #40`);
assertContains(registry, "verify-region-admin-boundary.mjs", `${registryPath}: central readiness board must mention the fast boundary guardrail`);
assertContains(packageJson, "test:verify:region:admin-boundary", `${packagePath}: package scripts must expose region boundary fixture tests`);
assertContains(packageJson, "test:verify:ffa:ui:migration", `${packagePath}: package scripts must expose aggregate FFA fixture tests`);
assertContains(packageJson, "npm run test:verify:region:admin-boundary", `${packagePath}: aggregate FFA fixture tests must include region boundary fixtures`);
assertContains(verifierTest, "region admin boundary verifier passes canonical fixture", `${verifierTestPath}: fixture tests must include canonical pass case`);
assertContains(verifierTest, "rejects stale central readiness board", `${verifierTestPath}: fixture tests must include docs sync negative case`);

if (failures.length > 0) {
  console.error("region admin boundary verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("region admin boundary verification passed");

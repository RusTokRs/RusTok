#!/usr/bin/env node
// RusTok tenant admin FFA boundary guardrails.
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

const libPath = "crates/rustok-tenant/admin/src/lib.rs";
const corePath = "crates/rustok-tenant/admin/src/core.rs";
const uiPath = "crates/rustok-tenant/admin/src/ui/leptos.rs";
const transportModPath = "crates/rustok-tenant/admin/src/transport/mod.rs";
const nativeAdapterPath = "crates/rustok-tenant/admin/src/transport/native_server_adapter.rs";

for (const filePath of [libPath, corePath, uiPath, transportModPath, nativeAdapterPath]) {
  assertExists(filePath, `${filePath}: expected tenant admin FFA boundary file`);
}
assertMissing(
  "crates/rustok-tenant/admin/src/api.rs",
  "crates/rustok-tenant/admin/src/api.rs: pre-FFA api facade must stay removed",
);

const lib = readRepo(libPath);
const core = readRepo(corePath);
const ui = readRepo(uiPath);
const transportMod = readRepo(transportModPath);
const nativeAdapter = readRepo(nativeAdapterPath);

assertContains(lib, "mod core;", `${libPath}: crate root must wire core`);
assertContains(lib, "mod transport;", `${libPath}: crate root must wire transport facade`);
assertContains(lib, "mod ui;", `${libPath}: crate root must wire UI adapters`);
assertContains(lib, "pub use ui::leptos::TenantAdmin;", `${libPath}: crate root must re-export the Leptos adapter surface`);
assertNotContains(lib, "mod api;", `${libPath}: crate root must not wire the pre-FFA api facade`);

for (const marker of ["leptos::", "leptos_", "#[component]", "#[server]", "LocalResource"]) {
  assertNotContains(core, marker, `${corePath}: core must stay Leptos/server-function free (${marker})`);
}
assertContains(core, "TenantAdminInfoCards", `${corePath}: core must own tenant info-card view-model policy`);
assertContains(core, "load_bootstrap_error_message", `${corePath}: core must own transport-agnostic load-error formatting`);

assertContains(ui, "use crate::{core, i18n::t, transport};", `${uiPath}: Leptos adapter must consume core and transport facade`);
assertContains(ui, "transport::fetch_bootstrap", `${uiPath}: Leptos adapter must call module-owned transport facade`);
for (const marker of ["mod api;", "crate::api", /(^|[^A-Za-z0-9_])api::/, "native_server_adapter::", "#[server"] ) {
  assertNotContains(ui, marker, `${uiPath}: UI adapter must not call raw/pre-FFA transport (${marker})`);
}

assertContains(transportMod, "pub mod native_server_adapter;", `${transportModPath}: transport facade must wire native adapter`);
assertContains(transportMod, "native_server_adapter::tenant_bootstrap_native", `${transportModPath}: facade must call native bootstrap path`);
assertNotContains(transportMod, "#[server", `${transportModPath}: server-function endpoint belongs in native_server_adapter.rs`);

assertContains(nativeAdapter, "#[server", `${nativeAdapterPath}: native adapter must contain server-function endpoint`);
assertContains(nativeAdapter, "tenant_bootstrap_native", `${nativeAdapterPath}: native adapter must own bootstrap server-function endpoint`);

if (failures.length > 0) {
  console.error("tenant admin boundary verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("tenant admin boundary verification passed");

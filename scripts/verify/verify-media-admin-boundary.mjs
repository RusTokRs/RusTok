#!/usr/bin/env node
// RusTok media admin FFA boundary guardrails.
// Fast source-level checks for the module-owned core/transport/ui split.

import { existsSync, readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = process.env.RUSTOK_VERIFY_REPO_ROOT
  ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT)
  : path.resolve(scriptDir, "../..");
const failures = [];

function repoPath(relativePath) { return path.join(repoRoot, relativePath); }
function readRepo(relativePath) { return readFileSync(repoPath(relativePath), "utf8"); }
function fail(message) { failures.push(message); }
function assertExists(relativePath, description) { if (!existsSync(repoPath(relativePath))) fail(description); }
function assertContains(text, pattern, description) {
  const found = typeof pattern === "string" ? text.includes(pattern) : pattern.test(text);
  if (!found) fail(description);
}
function assertNotContains(text, pattern, description) {
  const found = typeof pattern === "string" ? text.includes(pattern) : pattern.test(text);
  if (found) fail(description);
}

const libPath = "crates/rustok-media/admin/src/lib.rs";
const corePath = "crates/rustok-media/admin/src/core.rs";
const transportModPath = "crates/rustok-media/admin/src/transport/mod.rs";
const nativeAdapterPath = "crates/rustok-media/admin/src/transport/native_server_adapter.rs";
const graphqlAdapterPath = "crates/rustok-media/admin/src/transport/graphql_adapter.rs";
const restAdapterPath = "crates/rustok-media/admin/src/transport/rest_adapter.rs";
const uiPath = "crates/rustok-media/admin/src/ui/leptos.rs";
const cargoPath = "crates/rustok-media/admin/Cargo.toml";
const localPlanPath = "crates/rustok-media/docs/implementation-plan.md";
const registryPath = "docs/modules/registry.md";

for (const filePath of [
  libPath,
  corePath,
  transportModPath,
  nativeAdapterPath,
  graphqlAdapterPath,
  restAdapterPath,
  uiPath,
  cargoPath,
  localPlanPath,
  registryPath,
]) {
  assertExists(filePath, `${filePath}: expected media admin FFA boundary file`);
}

const lib = readRepo(libPath);
const core = readRepo(corePath);
const transportMod = readRepo(transportModPath);
const nativeAdapter = readRepo(nativeAdapterPath);
const graphqlAdapter = readRepo(graphqlAdapterPath);
const restAdapter = readRepo(restAdapterPath);
const ui = readRepo(uiPath);
const cargoToml = readRepo(cargoPath);
const localPlan = readRepo(localPlanPath);
const registry = readRepo(registryPath);

assertContains(lib, "mod core;", `${libPath}: crate root must wire core`);
assertContains(lib, "mod transport;", `${libPath}: crate root must wire transport facade`);
assertContains(lib, "mod ui;", `${libPath}: crate root must wire UI adapters`);
assertContains(lib, "pub use ui::leptos::MediaAdmin;", `${libPath}: crate root must re-export the Leptos adapter surface`);

for (const marker of ["leptos::", "leptos_", "#[component]", /#\[server\s*\]/, "LocalResource", "GraphqlHttpError"]) {
  assertNotContains(core, marker, `${corePath}: core must stay framework/transport-runtime free (${marker})`);
}
for (const marker of [
  "MediaAdminBusyKey",
  "media_upload_success_state",
  "media_list_card_view_model",
  "media_detail_lines",
  "selected_translation_form_state",
  "media_usage_stat_cards",
  "media_admin_context_error",
]) {
  assertContains(core, marker, `${corePath}: core must own ${marker} policy/helper`);
}
assertContains(core, "#[cfg(test)]", `${corePath}: core helpers must keep framework-free unit coverage`);

assertContains(transportMod, "mod native_server_adapter;", `${transportModPath}: transport facade must wire native adapter`);
assertContains(transportMod, "mod graphql_adapter;", `${transportModPath}: transport facade must wire GraphQL adapter`);
assertContains(transportMod, "mod rest_adapter;", `${transportModPath}: transport facade must wire REST adapter`);
assertContains(transportMod, "native_server_adapter::media_library_native", `${transportModPath}: facade must prefer native media library path`);
assertContains(transportMod, "graphql_adapter::fetch_media_library_graphql", `${transportModPath}: facade must retain GraphQL fallback`);
assertContains(transportMod, "rest_adapter::upload_media_rest", `${transportModPath}: facade must keep upload on REST adapter`);
assertNotContains(transportMod, "#[server", `${transportModPath}: server-function endpoints belong in native_server_adapter.rs`);

assertContains(nativeAdapter, "#[server", `${nativeAdapterPath}: native adapter must contain server-function endpoints`);
assertContains(nativeAdapter, "media_library_native", `${nativeAdapterPath}: native adapter must expose library endpoint`);
assertContains(nativeAdapter, "MediaService", `${nativeAdapterPath}: native adapter must call module-owned service layer`);
assertContains(nativeAdapter, "HostRuntimeContext", `${nativeAdapterPath}: native adapter must consume neutral host runtime context`);
assertContains(nativeAdapter, "shared_get::<rustok_storage::StorageRuntime>()", `${nativeAdapterPath}: native adapter must receive storage through neutral host runtime context`);
assertContains(graphqlAdapter, "query MediaLibrary", `${graphqlAdapterPath}: GraphQL adapter must retain media library headless contract`);
assertContains(restAdapter, "/api/media", `${restAdapterPath}: REST adapter must retain upload endpoint`);

assertContains(ui, "use crate::core::{", `${uiPath}: Leptos adapter must consume core helpers`);
assertContains(ui, "use crate::transport;", `${uiPath}: Leptos adapter must consume transport facade`);
assertContains(ui, "transport::fetch_media_library", `${uiPath}: Leptos adapter must call module-owned transport facade`);
assertContains(ui, "transport::upload_media", `${uiPath}: Leptos adapter must call REST upload via facade`);
for (const marker of ["native_server_adapter::", "graphql_adapter::", "rest_adapter::"] ) {
  assertNotContains(ui, marker, `${uiPath}: UI adapter must not call raw transport adapter (${marker})`);
}

assertContains(localPlan, "verify-media-admin-boundary.mjs", `${localPlanPath}: local plan must record fast boundary guardrail evidence`);
assertContains(localPlan, "HostRuntimeContext", `${localPlanPath}: local plan must record host-neutral native admin transport`);
assertContains(registry, "verify-media-admin-boundary.mjs", `${registryPath}: central registry must record media admin boundary guardrail`);
assertContains(registry, "HostRuntimeContext", `${registryPath}: central registry must record host-neutral media admin transport`);

if (failures.length > 0) {
  console.error("Media admin boundary verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Media admin boundary verification passed");

#!/usr/bin/env node
// RusTok SEO admin FFA boundary guardrails.
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
function assertMissing(relativePath, description) { if (existsSync(repoPath(relativePath))) fail(description); }
function assertContains(text, pattern, description) {
  const found = typeof pattern === "string" ? text.includes(pattern) : pattern.test(text);
  if (!found) fail(description);
}
function assertNotContains(text, pattern, description) {
  const found = typeof pattern === "string" ? text.includes(pattern) : pattern.test(text);
  if (found) fail(description);
}

const libPath = "crates/rustok-seo/admin/src/lib.rs";
const corePath = "crates/rustok-seo/admin/src/core.rs";
const transportModPath = "crates/rustok-seo/admin/src/transport/mod.rs";
const nativeAdapterPath = "crates/rustok-seo/admin/src/transport/native_server_adapter.rs";
const cargoPath = "crates/rustok-seo/admin/Cargo.toml";
const uiPath = "crates/rustok-seo/admin/src/ui/leptos.rs";
const defaultsSectionPath = "crates/rustok-seo/admin/src/sections/defaults.rs";
const localPlanPath = "crates/rustok-seo/docs/implementation-plan.md";
const registryPath = "docs/modules/registry.md";

for (const filePath of [libPath, corePath, transportModPath, nativeAdapterPath, cargoPath, uiPath, defaultsSectionPath, localPlanPath, registryPath]) {
  assertExists(filePath, `${filePath}: expected SEO admin FFA boundary file`);
}
assertMissing("crates/rustok-seo/admin/src/transport.rs", "crates/rustok-seo/admin/src/transport.rs: monolithic pre-split transport facade must stay removed");

const lib = readRepo(libPath);
const core = readRepo(corePath);
const transportMod = readRepo(transportModPath);
const nativeAdapter = readRepo(nativeAdapterPath);
const cargo = readRepo(cargoPath);
const ui = readRepo(uiPath);
const defaultsSection = readRepo(defaultsSectionPath);
const localPlan = readRepo(localPlanPath);
const registry = readRepo(registryPath);

assertContains(lib, "mod core;", `${libPath}: crate root must wire core`);
assertContains(lib, "mod transport;", `${libPath}: crate root must wire transport facade`);
assertContains(lib, "mod ui;", `${libPath}: crate root must wire UI adapters`);
assertContains(lib, "pub use ui::leptos::SeoAdmin;", `${libPath}: crate root must re-export the Leptos adapter surface`);

for (const marker of ["leptos::", "leptos_", "#[component]", /#\[server\s*\]/, "LocalResource"]) {
  assertNotContains(core, marker, `${corePath}: core must stay Leptos/server-function free (${marker})`);
}
for (const marker of [
  "SeoAdminBusyKey",
  "SeoAdminTab",
  "SeoBulkFilterForm",
  "SeoBulkActionForm",
  "SeoRedirectForm",
  "build_input",
  "validate_sitemap_generation_enabled",
  "format_index_repair_replay_result",
  "SeoSettingsSnapshotItem",
  "build_seo_settings_snapshot_items",
]) {
  assertContains(core, marker, `${corePath}: core must own ${marker} policy/helper`);
}

assertContains(transportMod, "mod native_server_adapter;", `${transportModPath}: transport facade must wire native adapter`);
assertContains(transportMod, "use native_server_adapter::{", `${transportModPath}: facade must import native endpoints through adapter boundary`);
assertContains(transportMod, "seo_redirects_native", `${transportModPath}: facade must dispatch redirects through native adapter`);
assertContains(transportMod, "seo_index_repair_replay_native", `${transportModPath}: facade must dispatch index repair through native adapter`);
assertContains(transportMod, "normalize_preview_bulk_apply_input", `${transportModPath}: facade must retain transport-agnostic request normalization`);
assertNotContains(transportMod, /#\[server\s*\]/, `${transportModPath}: server-function endpoints belong in native_server_adapter.rs`);
assertNotContains(transportMod, "expect_context::<AppContext>", `${transportModPath}: host context extraction belongs in native_server_adapter.rs`);

assertContains(nativeAdapter, /#\[server[^\n]*endpoint = "seo\/redirects"/, `${nativeAdapterPath}: native adapter must own redirect server-function endpoint`);
assertContains(nativeAdapter, /#\[server[^\n]*endpoint = "seo\/index-repair-replay"/, `${nativeAdapterPath}: native adapter must own index repair/replay server-function endpoint`);
assertContains(nativeAdapter, "seo_service_from_context", `${nativeAdapterPath}: native adapter must own host context extraction`);
assertContains(nativeAdapter, "persist_seo_settings", `${nativeAdapterPath}: native adapter must own settings persistence helper`);
assertContains(nativeAdapter, "expect_context::<HostRuntimeContext>()", `${nativeAdapterPath}: native adapter must consume neutral host runtime context`);
assertContains(nativeAdapter, "shared_get::<TransactionalEventBus>()", `${nativeAdapterPath}: native adapter must read the typed event bus from host runtime context`);
assertContains(nativeAdapter, "shared_get::<std::sync::Arc<ModuleRuntimeExtensions>>()", `${nativeAdapterPath}: native adapter must read SEO runtime extensions from typed host handles`);
assertContains(nativeAdapter, "runtime_ctx.db_clone()", `${nativeAdapterPath}: native adapter must read DB from neutral host runtime context`);
assertNotContains(nativeAdapter, "loco_rs", `${nativeAdapterPath}: native adapter must not depend on Loco runtime context`);
assertNotContains(nativeAdapter, "rustok_outbox::loco", `${nativeAdapterPath}: native adapter must not consume outbox Loco adapter`);
assertNotContains(cargo, "loco-rs", `${cargoPath}: SEO admin crate must not depend on Loco`);
assertNotContains(cargo, "loco-adapter", `${cargoPath}: SEO admin crate must not enable outbox Loco adapter feature`);

assertContains(ui, "use crate::transport;", `${uiPath}: Leptos adapter must consume transport facade`);
assertContains(ui, "transport::fetch_redirects", `${uiPath}: Leptos adapter must call module-owned transport facade`);
assertContains(ui, "transport::queue_bulk_apply", `${uiPath}: Leptos adapter must call bulk mutations through transport facade`);
assertContains(defaultsSection, "build_seo_settings_snapshot_items", `${defaultsSectionPath}: defaults section must consume core-owned settings snapshot view-model policy`);
for (const marker of ["native_server_adapter::", "seo_redirects_native", "seo_index_repair_replay_native"]) {
  assertNotContains(ui, marker, `${uiPath}: UI adapter must not call raw native endpoints (${marker})`);
}

assertContains(localPlan, "verify-seo-admin-boundary.mjs", `${localPlanPath}: local plan must record fast boundary guardrail evidence`);
assertContains(registry, "verify-seo-admin-boundary.mjs", `${registryPath}: central registry must record SEO admin boundary guardrail`);

if (failures.length > 0) {
  console.error("SEO admin boundary verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("SEO admin boundary verification passed");

#!/usr/bin/env node
// RusTok comments admin FFA boundary guardrails.
// Fast source-level checks for the module-owned core/transport/ui split and
// the documented native-only comments admin transport exception.

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

const libPath = "crates/rustok-comments/admin/src/lib.rs";
const corePath = "crates/rustok-comments/admin/src/core.rs";
const uiPath = "crates/rustok-comments/admin/src/ui/leptos.rs";
const transportModPath = "crates/rustok-comments/admin/src/transport/mod.rs";
const nativeAdapterPath = "crates/rustok-comments/admin/src/transport/native_server_adapter.rs";
const cargoPath = "crates/rustok-comments/admin/Cargo.toml";
const localPlanPath = "crates/rustok-comments/docs/implementation-plan.md";
const registryPath = "docs/modules/registry.md";

for (const filePath of [
  libPath,
  corePath,
  uiPath,
  transportModPath,
  nativeAdapterPath,
  cargoPath,
  localPlanPath,
  registryPath,
]) {
  assertExists(filePath, `${filePath}: expected comments admin FFA boundary file`);
}
assertMissing(
  "crates/rustok-comments/admin/src/api.rs",
  "crates/rustok-comments/admin/src/api.rs: pre-FFA api facade must stay removed",
);

const lib = readRepo(libPath);
const core = readRepo(corePath);
const ui = readRepo(uiPath);
const transportMod = readRepo(transportModPath);
const nativeAdapter = readRepo(nativeAdapterPath);
const cargoToml = readRepo(cargoPath);
const localPlan = readRepo(localPlanPath);
const registry = readRepo(registryPath);

assertContains(lib, "mod core;", `${libPath}: crate root must wire core`);
assertContains(lib, "mod transport;", `${libPath}: crate root must wire transport facade`);
assertContains(lib, "mod ui;", `${libPath}: crate root must wire UI adapters`);
assertContains(lib, "pub use ui::leptos::CommentsAdmin;", `${libPath}: crate root must re-export the Leptos adapter surface`);
assertNotContains(lib, "mod api;", `${libPath}: crate root must not wire the pre-FFA api facade`);

for (const marker of ["leptos::", "leptos_", "#[component]", "#[server]", "LocalResource"]) {
  assertNotContains(core, marker, `${corePath}: core must stay Leptos/server-function free (${marker})`);
}
for (const marker of [
  "UiRouteQueryUpdate",
  "comments_admin_select_thread_query_write",
  "comments_admin_locale_query_write",
  "COMMENTS_ADMIN_THREAD_QUERY_KEY",
  "COMMENTS_ADMIN_LOCALE_QUERY_KEY",
]) {
  assertContains(core, marker, `${corePath}: core must own comments admin route/query policy (${marker})`);
}
assertContains(core, "CommentThreadsRequest", `${corePath}: core must own thread-list request construction`);
assertContains(core, "SetCommentStatusCommand", `${corePath}: core must own comment-status command construction`);

assertContains(ui, "use crate::core::", `${uiPath}: Leptos adapter must consume core helpers`);
assertContains(ui, "use crate::transport;", `${uiPath}: Leptos adapter must consume transport facade`);
assertContains(ui, "transport::fetch_threads", `${uiPath}: Leptos adapter must call module-owned transport facade`);
assertContains(ui, "apply_comments_route_query_update", `${uiPath}: Leptos adapter must apply prepared route-query writes`);
for (const marker of [
  "AdminQueryKey",
  "push_value(",
  "replace_value(",
  "clear_key(",
  "crate::api",
  /(^|[^A-Za-z0-9_])api::/,
  "native_server_adapter::",
  "#[server",
]) {
  assertNotContains(ui, marker, `${uiPath}: UI adapter must not own raw route/transport policy (${marker})`);
}

assertContains(transportMod, "pub(crate) mod native_server_adapter;", `${transportModPath}: transport facade must wire native adapter`);
assertContains(transportMod, "CommentsAdminTransportPath", `${transportModPath}: transport facade must document active transport path`);
assertContains(transportMod, "ACTIVE_TRANSPORT_PATH", `${transportModPath}: transport facade must expose the native-only path marker`);
assertContains(transportMod, "native_server_adapter::fetch_threads", `${transportModPath}: facade must call native thread list path`);
assertNotContains(transportMod, "#[server", `${transportModPath}: server-function endpoints belong in native_server_adapter.rs`);
assertNotContains(transportMod, "graphql", `${transportModPath}: comments admin must not invent a package-local GraphQL fallback`);

assertContains(nativeAdapter, "#[server", `${nativeAdapterPath}: native adapter must contain server-function endpoints`);
assertContains(nativeAdapter, "CommentsService::new", `${nativeAdapterPath}: native adapter must call CommentsService`);
assertContains(nativeAdapter, "HostRuntimeContext", `${nativeAdapterPath}: native adapter must consume neutral host runtime context`);
assertNotContains(nativeAdapter, "loco_rs", `${nativeAdapterPath}: native adapter must not depend on Loco runtime context`);
assertContains(nativeAdapter, "comments_threads_native", `${nativeAdapterPath}: native adapter must own thread list server function`);
assertContains(nativeAdapter, "comments_set_comment_status_native", `${nativeAdapterPath}: native adapter must own comment status server function`);
assertNotContains(cargoToml, "loco-rs", `${cargoPath}: comments admin must not depend on Loco`);

assertContains(localPlan, "native-only comments admin exception", `${localPlanPath}: local plan must document native-only exception`);
assertContains(localPlan, "Loco-free native admin transport", `${localPlanPath}: local plan must record Loco-free native transport evidence`);
assertContains(localPlan, "UiRouteQueryUpdate", `${localPlanPath}: local plan must document shared route-query contract`);
assertContains(localPlan, "verify-comments-admin-boundary.mjs", `${localPlanPath}: local plan must record fast boundary guardrail evidence`);
assertContains(registry, "verify-comments-admin-boundary.mjs", `${registryPath}: central registry must record comments admin boundary guardrail`);

if (failures.length > 0) {
  console.error("comments admin boundary verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("comments admin boundary verification passed");

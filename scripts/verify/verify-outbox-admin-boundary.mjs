#!/usr/bin/env node
// RusTok outbox admin FFA boundary guardrails.
// Fast source-level checks for the read-only module-owned core/transport/ui split.

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

const libPath = "crates/rustok-outbox/admin/src/lib.rs";
const corePath = "crates/rustok-outbox/admin/src/core.rs";
const uiPath = "crates/rustok-outbox/admin/src/ui/leptos.rs";
const transportModPath = "crates/rustok-outbox/admin/src/transport/mod.rs";
const nativeAdapterPath = "crates/rustok-outbox/admin/src/transport/native_server_adapter.rs";
const localPlanPath = "crates/rustok-outbox/docs/implementation-plan.md";
const localDocsPath = "crates/rustok-outbox/docs/README.md";
const registryPath = "docs/modules/registry.md";

for (const filePath of [libPath, corePath, uiPath, transportModPath, nativeAdapterPath, localPlanPath, localDocsPath, registryPath]) {
  assertExists(filePath, `${filePath}: expected outbox admin boundary file`);
}
assertMissing("crates/rustok-outbox/admin/src/api.rs", "crates/rustok-outbox/admin/src/api.rs: pre-FFA api facade must stay removed");

const lib = readRepo(libPath);
const core = readRepo(corePath);
const ui = readRepo(uiPath);
const transportMod = readRepo(transportModPath);
const nativeAdapter = readRepo(nativeAdapterPath);
const localPlan = readRepo(localPlanPath);
const localDocs = readRepo(localDocsPath);
const registry = readRepo(registryPath);

assertContains(lib, "mod core;", `${libPath}: crate root must wire core`);
assertContains(lib, "mod transport;", `${libPath}: crate root must wire transport facade`);
assertContains(lib, "mod ui;", `${libPath}: crate root must wire UI adapters`);
assertContains(lib, "pub use ui::leptos::OutboxAdmin;", `${libPath}: crate root must re-export the Leptos adapter surface`);
assertNotContains(lib, "mod api;", `${libPath}: crate root must not wire the pre-FFA api facade`);

for (const marker of ["leptos::", "leptos_", "#[component]", "#[server]", "LocalResource", "ServerFnError"]) {
  assertNotContains(core, marker, `${corePath}: core must stay Leptos/server-function free (${marker})`);
}
for (const marker of ["OutboxAdminBootstrap", "OutboxAdminShellText", "OutboxInfoCardViewModel", "outbox_info_cards"]) {
  assertContains(core, marker, `${corePath}: core must own outbox bootstrap/view-model policy (${marker})`);
}

assertContains(ui, "use crate::core::{", `${uiPath}: Leptos adapter must consume core helpers`);
assertContains(ui, "use crate::transport;", `${uiPath}: Leptos adapter must consume transport facade`);
assertContains(ui, "transport::fetch_bootstrap", `${uiPath}: Leptos adapter must call module-owned transport facade`);
assertContains(ui, "outbox_info_cards", `${uiPath}: Leptos adapter must render core-owned cards`);
for (const marker of ["crate::api", /(^|[^A-Za-z0-9_])api::/, "native_server_adapter::", "fetch_bootstrap_native", "#[server"] ) {
  assertNotContains(ui, marker, `${uiPath}: UI adapter must not call raw/pre-FFA transport (${marker})`);
}

assertContains(transportMod, "mod native_server_adapter;", `${transportModPath}: transport facade must wire native adapter`);
assertContains(transportMod, "OutboxTransportError", `${transportModPath}: transport facade must expose typed transport error`);
assertContains(transportMod, "fetch_bootstrap", `${transportModPath}: transport facade must expose bootstrap loader`);
assertContains(transportMod, "native_server_adapter::fetch_bootstrap_native", `${transportModPath}: facade must call native bootstrap path`);
assertNotContains(transportMod, "#[server", `${transportModPath}: server-function endpoint belongs in native_server_adapter.rs`);
assertNotContains(transportMod, "graphql", `${transportModPath}: outbox admin must not invent a package-local GraphQL fallback`);

assertContains(nativeAdapter, "#[server", `${nativeAdapterPath}: native adapter must contain server-function endpoint`);
assertContains(nativeAdapter, "outbox_bootstrap_native", `${nativeAdapterPath}: native adapter must own bootstrap server function`);
assertContains(nativeAdapter, "OutboxModule", `${nativeAdapterPath}: native adapter must bootstrap through the outbox module`);
assertContains(nativeAdapter, "relay_notes", `${nativeAdapterPath}: native adapter must expose relay/backlog visibility notes`);

assertContains(localPlan, "verify-outbox-admin-boundary.mjs", `${localPlanPath}: local plan must record fast boundary guardrail evidence`);
assertContains(localDocs, "verify-outbox-admin-boundary.mjs", `${localDocsPath}: local docs must list the no-compile boundary guardrail`);
assertContains(registry, "verify-outbox-admin-boundary.mjs", `${registryPath}: central registry must record outbox admin boundary guardrail`);

if (failures.length > 0) {
  console.error("outbox admin boundary verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("outbox admin boundary verification passed");

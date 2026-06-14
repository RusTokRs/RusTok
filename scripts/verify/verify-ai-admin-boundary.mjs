#!/usr/bin/env node
// RusTok AI admin FFA boundary guardrails for the first core slice.

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

const libPath = "crates/rustok-ai/admin/src/lib.rs";
const uiPath = "crates/rustok-ai/admin/src/ui/leptos.rs";
const corePath = "crates/rustok-ai/admin/src/core.rs";
const transportModPath = "crates/rustok-ai/admin/src/transport/mod.rs";
const nativeAdapterPath = "crates/rustok-ai/admin/src/transport/native_server_adapter.rs";
const graphqlAdapterPath = "crates/rustok-ai/admin/src/transport/graphql_adapter.rs";

assertExists(libPath, `${libPath}: expected AI admin crate root file`);
assertExists(uiPath, `${uiPath}: expected AI admin Leptos adapter file`);
assertExists(corePath, `${corePath}: expected AI admin core slice file`);
assertExists(transportModPath, `${transportModPath}: expected AI admin transport facade file`);
assertExists(nativeAdapterPath, `${nativeAdapterPath}: expected AI admin native server adapter file`);
assertExists(graphqlAdapterPath, `${graphqlAdapterPath}: expected AI admin GraphQL/headless adapter file`);
if (existsSync(repoPath("crates/rustok-ai/admin/src/api.rs"))) {
  fail("crates/rustok-ai/admin/src/api.rs: pre-FFA api facade must stay removed");
}

const lib = readRepo(libPath);
const ui = readRepo(uiPath);
const core = readRepo(corePath);
const transportMod = readRepo(transportModPath);
const nativeAdapter = readRepo(nativeAdapterPath);
const graphqlAdapter = readRepo(graphqlAdapterPath);

assertContains(lib, "mod core;", `${libPath}: crate root must wire core`);
assertContains(lib, "pub mod transport;", `${libPath}: crate root must expose the transport facade for headless adapters`);
assertContains(lib, "mod ui;", `${libPath}: crate root must wire UI adapters`);
assertContains(lib, "pub use ui::leptos::AiAdmin;", `${libPath}: crate root must re-export the Leptos adapter surface`);
assertContains(ui, "use crate::core::{", `${uiPath}: Leptos adapter must import core-owned helpers`);
assertContains(ui, "summarize_recent_runs", `${uiPath}: Leptos adapter must consume core-owned diagnostics summary policy`);
assertContains(ui, "average_latency_ms", `${uiPath}: Leptos adapter must consume core-owned latency policy`);
assertContains(ui, "product_attributes_task_payload", `${uiPath}: Leptos adapter must call core-owned payload builder`);
assertNotContains(ui, "fn product_attributes_task_payload", `${uiPath}: payload builder must not live in the Leptos adapter`);
assertNotContains(ui, "fn summarize_recent_runs", `${uiPath}: diagnostics summary policy must not live in the Leptos adapter`);
assertNotContains(ui, "fn average_latency_ms", `${uiPath}: latency fallback policy must not live in the Leptos adapter`);
assertNotContains(ui, "fn parse_csv(value: String)", `${uiPath}: CSV request normalization must not live in the Leptos adapter`);
assertNotContains(ui, /t\(\s*locale,\s*locale,/, `${uiPath}: i18n helper calls must not pass locale twice`);
assertContains(ui, "transport::fetch_bootstrap", `${uiPath}: Leptos adapter must call the AI transport facade`);
assertNotContains(ui, /(^|[^A-Za-z0-9_])api::/, `${uiPath}: Leptos adapter must not call the raw pre-FFA api module`);

for (const marker of ["leptos::", "leptos_", "#[component]", "#[server]", "RwSignal", "LocalResource", "web_sys::"]) {
  assertNotContains(core, marker, `${corePath}: core must stay UI/runtime free (${marker})`);
}
assertContains(transportMod, "pub mod native_server_adapter;", `${transportModPath}: transport facade must wire the native adapter`);
assertContains(transportMod, "pub mod graphql_adapter;", `${transportModPath}: transport facade must wire the GraphQL/headless adapter`);
assertContains(transportMod, "pub use native_server_adapter::{", `${transportModPath}: transport facade must re-export native adapter operations`);
assertContains(transportMod, "fetch_bootstrap", `${transportModPath}: transport facade must expose bootstrap loading`);
assertContains(transportMod, "run_task_job", `${transportModPath}: transport facade must expose direct job execution`);
assertContains(nativeAdapter, "#[server", `${nativeAdapterPath}: native adapter must contain server-function endpoints`);
assertContains(nativeAdapter, "ai_bootstrap_native", `${nativeAdapterPath}: native adapter must own bootstrap endpoint`);
assertContains(graphqlAdapter, "pub const AI_BOOTSTRAP_QUERY", `${graphqlAdapterPath}: GraphQL adapter must expose bootstrap query document`);
assertContains(graphqlAdapter, "pub fn bootstrap_request", `${graphqlAdapterPath}: GraphQL adapter must expose a bootstrap request builder`);
assertContains(graphqlAdapter, "pub fn session_request", `${graphqlAdapterPath}: GraphQL adapter must expose a session request builder`);
assertContains(graphqlAdapter, "pub fn session_events_subscription_request", `${graphqlAdapterPath}: GraphQL adapter must expose the subscription request builder`);
assertContains(graphqlAdapter, "pub fn graphql_ws_url_from_location", `${graphqlAdapterPath}: GraphQL adapter must own WebSocket endpoint policy`);
assertContains(graphqlAdapter, "pub fn connection_init_message", `${graphqlAdapterPath}: GraphQL adapter must own GraphQL WS connection-init payload construction`);
assertContains(graphqlAdapter, "pub fn session_events_subscribe_message", `${graphqlAdapterPath}: GraphQL adapter must own subscription message construction`);
assertNotContains(ui, "const AI_SESSION_EVENTS_SUBSCRIPTION", `${uiPath}: Leptos adapter must not duplicate the GraphQL subscription document`);
assertContains(graphqlAdapter, "aiRecentRuns(limit: 20)", `${graphqlAdapterPath}: GraphQL adapter must include recent persisted run diagnostics`);
assertContains(graphqlAdapter, "aiRecentRunStreamEvents(limit: 20)", `${graphqlAdapterPath}: GraphQL adapter must include recent stream diagnostics`);
for (const marker of ["leptos::", "#[component]", "#[server]", "RwSignal", "LocalResource", "web_sys::"]) {
  assertNotContains(graphqlAdapter, marker, `${graphqlAdapterPath}: GraphQL adapter must stay UI/runtime free (${marker})`);
}

for (const marker of [
  "pub fn parse_csv",
  "pub fn optional_text",
  "pub fn alloy_task_payload",
  "pub fn image_task_payload",
  "pub fn product_task_payload",
  "pub fn product_attributes_task_payload",
  "pub fn blog_task_payload",
  "pub fn average_latency_ms",
  "pub fn summarize_recent_runs",
]) {
  assertContains(core, marker, `${corePath}: expected core-owned helper ${marker}`);
}

if (failures.length > 0) {
  console.error("AI admin boundary verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("AI admin boundary verification passed");

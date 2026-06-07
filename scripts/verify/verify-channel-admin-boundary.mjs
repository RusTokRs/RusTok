#!/usr/bin/env node
// RusTok channel admin FFA boundary guardrails.
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
  if (!existsSync(repoPath(relativePath))) {
    fail(description);
  }
}

function assertMissing(relativePath, description) {
  if (existsSync(repoPath(relativePath))) {
    fail(description);
  }
}

function assertContains(text, pattern, description) {
  const found = typeof pattern === "string" ? text.includes(pattern) : pattern.test(text);
  if (!found) {
    fail(description);
  }
}

function assertNotContains(text, pattern, description) {
  const found = typeof pattern === "string" ? text.includes(pattern) : pattern.test(text);
  if (found) {
    fail(description);
  }
}

function assertChannelAdminBoundary() {
  const libPath = "crates/rustok-channel/admin/src/lib.rs";
  const corePath = "crates/rustok-channel/admin/src/core.rs";
  const uiPath = "crates/rustok-channel/admin/src/ui/leptos.rs";
  const transportModPath = "crates/rustok-channel/admin/src/transport/mod.rs";
  const nativeAdapterPath = "crates/rustok-channel/admin/src/transport/native_server_adapter.rs";
  const restAdapterPath = "crates/rustok-channel/admin/src/transport/rest_adapter.rs";

  for (const path of [libPath, corePath, uiPath, transportModPath, nativeAdapterPath, restAdapterPath]) {
    assertExists(path, `${path}: expected channel admin FFA boundary file`);
  }
  assertMissing(
    "crates/rustok-channel/admin/src/api.rs",
    "crates/rustok-channel/admin/src/api.rs: pre-FFA api facade must stay removed",
  );
  assertMissing(
    "crates/rustok-channel/admin/src/transport.rs",
    "crates/rustok-channel/admin/src/transport.rs: transport must remain split into transport/ adapters",
  );

  const lib = readRepo(libPath);
  const core = readRepo(corePath);
  const ui = readRepo(uiPath);
  const transportMod = readRepo(transportModPath);
  const nativeAdapter = readRepo(nativeAdapterPath);
  const restAdapter = readRepo(restAdapterPath);

  assertContains(lib, "mod core;", `${libPath}: crate root must wire core`);
  assertContains(lib, "mod transport;", `${libPath}: crate root must wire transport facade`);
  assertContains(lib, "mod ui;", `${libPath}: crate root must wire UI adapters`);
  assertContains(lib, "pub use ui::leptos::ChannelAdmin;", `${libPath}: crate root must re-export the Leptos adapter surface`);
  assertNotContains(lib, "mod api;", `${libPath}: crate root must not wire the pre-FFA api facade`);

  for (const marker of ["leptos::", "leptos_", "#[component]", "#[server]", "LocalResource"] ) {
    assertNotContains(core, marker, `${corePath}: core must stay Leptos/server-function free (${marker})`);
  }
  assertContains(core, "channel_selection_exists", `${corePath}: core must own selected-channel route cleanup policy`);

  assertContains(ui, "use crate::transport;", `${uiPath}: Leptos adapter must call the module-owned transport facade`);
  assertContains(ui, "channel_selection_exists", `${uiPath}: Leptos adapter must consume core-owned route selection policy`);
  for (const marker of [
    "mod api;",
    "crate::api",
    /(^|[^A-Za-z0-9_])api::/,
    "native_server_adapter::",
    "rest_adapter::",
    "reqwest::",
    "#[server",
  ]) {
    assertNotContains(ui, marker, `${uiPath}: UI adapter must not call raw/pre-FFA transport (${marker})`);
  }

  assertContains(transportMod, "mod native_server_adapter;", `${transportModPath}: transport facade must wire native server adapter`);
  assertContains(transportMod, "mod rest_adapter;", `${transportModPath}: transport facade must wire REST fallback adapter`);
  assertContains(transportMod, "native_server_adapter::channel_bootstrap_native", `${transportModPath}: facade must prefer native bootstrap path`);
  assertContains(transportMod, "rest_adapter::get_json", `${transportModPath}: facade must keep REST fallback path`);
  assertNotContains(transportMod, "#[server", `${transportModPath}: server-function endpoints belong in native_server_adapter.rs`);
  assertNotContains(transportMod, "reqwest::", `${transportModPath}: raw REST client belongs in rest_adapter.rs`);

  assertContains(nativeAdapter, "#[server", `${nativeAdapterPath}: native adapter must contain server-function endpoints`);
  assertContains(nativeAdapter, "channel_bootstrap_native", `${nativeAdapterPath}: native adapter must own bootstrap server-function endpoint`);
  assertNotContains(nativeAdapter, "reqwest::", `${nativeAdapterPath}: native adapter must not own REST fallback HTTP calls`);

  assertContains(restAdapter, "reqwest::Client::new", `${restAdapterPath}: REST adapter must own raw HTTP fallback calls`);
  assertContains(restAdapter, "RUSTOK_API_URL", `${restAdapterPath}: REST adapter must own external API base URL fallback`);
  assertNotContains(restAdapter, "#[server", `${restAdapterPath}: REST adapter must not contain server-function endpoints`);
}

assertChannelAdminBoundary();

if (failures.length > 0) {
  console.error("channel admin boundary verification failed:");
  for (const failure of failures) {
    console.error(`- ${failure}`);
  }
  process.exit(1);
}

console.log("channel admin boundary verification passed");

#!/usr/bin/env node
// RusTok workflow admin FFA boundary guardrails.
// Fast source-level checks for the module-owned core/transport/ui split.

import { existsSync, readdirSync, readFileSync } from "node:fs";
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

function readCoreSources(coreDirPath) {
  const absoluteCoreDir = repoPath(coreDirPath);
  if (!existsSync(absoluteCoreDir)) {
    return "";
  }

  return readdirSync(absoluteCoreDir)
    .filter((entry) => entry.endsWith(".rs"))
    .map((entry) => readRepo(path.posix.join(coreDirPath, entry)))
    .join("\n");
}

function assertWorkflowAdminBoundary() {
  const libPath = "crates/rustok-workflow/admin/src/lib.rs";
  const coreDirPath = "crates/rustok-workflow/admin/src/core";
  const uiPath = "crates/rustok-workflow/admin/src/ui/leptos.rs";
  const transportModPath = "crates/rustok-workflow/admin/src/transport/mod.rs";
  const nativeAdapterPath = "crates/rustok-workflow/admin/src/transport/native_server_adapter.rs";
  const graphqlAdapterPath = "crates/rustok-workflow/admin/src/transport/graphql_adapter.rs";
  const cargoPath = "crates/rustok-workflow/admin/Cargo.toml";

  for (const checkedPath of [
    libPath,
    `${coreDirPath}/mod.rs`,
    `${coreDirPath}/presentation.rs`,
    `${coreDirPath}/navigation.rs`,
    `${coreDirPath}/transport_context.rs`,
    `${coreDirPath}/error.rs`,
    `${coreDirPath}/command.rs`,
    uiPath,
    transportModPath,
    nativeAdapterPath,
    graphqlAdapterPath,
    cargoPath,
  ]) {
    assertExists(checkedPath, `${checkedPath}: expected workflow admin FFA boundary file`);
  }
  assertMissing(
    "crates/rustok-workflow/admin/src/api.rs",
    "crates/rustok-workflow/admin/src/api.rs: pre-FFA api facade must stay absent",
  );
  assertMissing(
    "crates/rustok-workflow/admin/src/transport.rs",
    "crates/rustok-workflow/admin/src/transport.rs: transport must remain split into transport/ adapters",
  );

  const lib = readRepo(libPath);
  const core = readCoreSources(coreDirPath);
  const ui = readRepo(uiPath);
  const transportMod = readRepo(transportModPath);
  const nativeAdapter = readRepo(nativeAdapterPath);
  const graphqlAdapter = readRepo(graphqlAdapterPath);
  const cargoToml = readRepo(cargoPath);

  assertContains(lib, "mod core;", `${libPath}: crate root must wire core`);
  assertContains(lib, "mod transport;", `${libPath}: crate root must wire transport facade`);
  assertContains(lib, "mod ui;", `${libPath}: crate root must wire UI adapters`);
  assertContains(lib, "pub use ui::leptos::WorkflowAdmin;", `${libPath}: crate root must re-export the Leptos adapter surface`);
  assertNotContains(lib, "mod api;", `${libPath}: crate root must not wire a pre-FFA api facade`);

  for (const marker of ["leptos::", "leptos_", "web_sys", "#[component]", "#[server", "LocalResource"] ) {
    assertNotContains(core, marker, `${coreDirPath}: core must stay Leptos/server-function free (${marker})`);
  }
  for (const marker of [
    "workflow_admin_nav_view_model",
    "WorkflowAdminTransportContext",
    "workflow_error_view_model",
    "workflow_template_create_command",
  ]) {
    assertContains(core, marker, `${coreDirPath}: core must own workflow admin ${marker} policy`);
  }

  assertContains(ui, "use crate::transport;", `${uiPath}: Leptos adapter must call the module-owned transport facade`);
  assertContains(ui, "workflow_admin_transport_context", `${uiPath}: Leptos adapter must consume core-owned transport context`);
  assertContains(ui, "workflow_admin_nav_view_model", `${uiPath}: Leptos adapter must consume core-owned route/navigation policy`);
  for (const marker of [
    "crate::api",
    /(^|[^A-Za-z0-9_])api::/,
    "graphql_adapter::",
    "native_server_adapter::",
    "rustok_graphql::",
    "execute_graphql",
    "#[server",
  ]) {
    assertNotContains(ui, marker, `${uiPath}: UI adapter must not call raw/pre-FFA transport (${marker})`);
  }

  assertContains(transportMod, "mod graphql_adapter;", `${transportModPath}: transport facade must wire GraphQL fallback adapter`);
  assertContains(transportMod, "mod native_server_adapter;", `${transportModPath}: transport facade must wire native server adapter`);
  assertContains(transportMod, "native_server_adapter::fetch_workflows_native", `${transportModPath}: facade must prefer native workflow list path`);
  assertContains(transportMod, "graphql_adapter::fetch_workflows", `${transportModPath}: facade must keep GraphQL workflow list fallback`);
  assertContains(transportMod, "WorkflowAdminTransportContext", `${transportModPath}: facade must accept core-owned context DTO`);
  assertContains(transportMod, "WorkflowTemplateCreateCommand", `${transportModPath}: facade must accept core-owned create command DTO`);
  assertNotContains(transportMod, "#[server", `${transportModPath}: server-function endpoints belong in native_server_adapter.rs`);
  assertNotContains(transportMod, "execute_graphql", `${transportModPath}: raw GraphQL execution belongs in graphql_adapter.rs`);

  assertContains(nativeAdapter, "#[server", `${nativeAdapterPath}: native adapter must contain server-function endpoints`);
  assertContains(nativeAdapter, "HostRuntimeContext", `${nativeAdapterPath}: native adapter must consume neutral host runtime context`);
  assertContains(nativeAdapter, "fetch_workflows_native", `${nativeAdapterPath}: native adapter must own workflow list endpoint`);
  assertContains(nativeAdapter, "fetch_templates_native", `${nativeAdapterPath}: native adapter must own template list endpoint`);
  assertContains(nativeAdapter, "create_from_template_native", `${nativeAdapterPath}: native adapter must own create-from-template endpoint`);
  assertNotContains(nativeAdapter, "loco_rs", `${nativeAdapterPath}: native adapter must not depend on Loco runtime context`);
  assertNotContains(nativeAdapter, "execute_graphql", `${nativeAdapterPath}: native adapter must not own GraphQL fallback calls`);
  assertNotContains(cargoToml, "loco-rs", `${cargoPath}: workflow admin must not depend on Loco`);

  assertContains(graphqlAdapter, "rustok_graphql", `${graphqlAdapterPath}: GraphQL adapter must own GraphQL client dependency`);
  assertContains(graphqlAdapter, "fetch_workflows", `${graphqlAdapterPath}: GraphQL adapter must expose workflow list fallback`);
  assertContains(graphqlAdapter, "fetch_templates", `${graphqlAdapterPath}: GraphQL adapter must expose template fallback`);
  assertContains(graphqlAdapter, "create_from_template", `${graphqlAdapterPath}: GraphQL adapter must expose create-from-template fallback`);
  assertNotContains(graphqlAdapter, "#[server", `${graphqlAdapterPath}: GraphQL adapter must not contain server-function endpoints`);
}

assertWorkflowAdminBoundary();

if (failures.length > 0) {
  console.error("workflow admin boundary verification failed:");
  for (const failure of failures) {
    console.error(`- ${failure}`);
  }
  process.exit(1);
}

console.log("workflow admin boundary verification passed");

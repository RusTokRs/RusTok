#!/usr/bin/env node
// Fast source-level guardrails for the rustok-outbox admin FFA boundary.
// This intentionally avoids Rust compilation and checks only the module-owned
// core/transport/ui split used by the read-only operator surface.

import { existsSync, readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = process.env.RUSTOK_VERIFY_REPO_ROOT
  ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT)
  : path.resolve(scriptDir, "../..");
const failures = [];

function readRepo(relativePath) {
  return readFileSync(path.join(repoRoot, relativePath), "utf8");
}

function fail(message) {
  failures.push(message);
}

function assertContains(text, pattern, description) {
  const found = typeof pattern === "string" ? text.includes(pattern) : pattern.test(text);
  if (!found) fail(description);
}

function assertNotContains(text, pattern, description) {
  const found = typeof pattern === "string" ? text.includes(pattern) : pattern.test(text);
  if (found) fail(description);
}

function rustFunctionBody(text, functionName) {
  const signature = new RegExp(`(?:pub(?:\\([^)]*\\))?\\s+)?(?:async\\s+)?fn\\s+${functionName}\\s*\\(`);
  const match = signature.exec(text);
  if (!match) {
    fail(`missing function ${functionName}`);
    return "";
  }
  const openBrace = text.indexOf("{", match.index);
  if (openBrace === -1) {
    fail(`missing body for function ${functionName}`);
    return "";
  }
  let depth = 0;
  for (let index = openBrace; index < text.length; index += 1) {
    const char = text[index];
    if (char === "{") depth += 1;
    if (char === "}") depth -= 1;
    if (depth === 0) return text.slice(openBrace, index + 1);
  }
  fail(`unterminated body for function ${functionName}`);
  return "";
}

function assertOutboxAdminBoundary() {
  const libPath = "crates/rustok-outbox/admin/src/lib.rs";
  const corePath = "crates/rustok-outbox/admin/src/core.rs";
  const transportPath = "crates/rustok-outbox/admin/src/transport/mod.rs";
  const nativePath = "crates/rustok-outbox/admin/src/transport/native_server_adapter.rs";
  const uiPath = "crates/rustok-outbox/admin/src/ui/leptos.rs";
  const planPath = "crates/rustok-outbox/docs/implementation-plan.md";
  const registryPath = "docs/modules/registry.md";

  for (const removedPath of [
    "crates/rustok-outbox/admin/src/api.rs",
    "crates/rustok-outbox/admin/src/transport.rs",
  ]) {
    if (existsSync(path.join(repoRoot, removedPath))) {
      fail(`${removedPath}: legacy pre-FFA admin transport path must stay absent`);
    }
  }

  const lib = readRepo(libPath);
  const core = readRepo(corePath);
  const transport = readRepo(transportPath);
  const native = readRepo(nativePath);
  const ui = readRepo(uiPath);
  const plan = readRepo(planPath);
  const registry = readRepo(registryPath);

  assertContains(lib, "mod core;", `${libPath}: must wire the Leptos-free core layer`);
  assertContains(lib, "mod transport;", `${libPath}: must wire the module-owned transport facade`);
  assertContains(lib, "pub mod ui;", `${libPath}: must expose only the UI adapter publicly`);
  assertNotContains(lib, "pub mod transport;", `${libPath}: transport facade must not become the public UI API`);

  for (const marker of ["leptos::", "#[component]", "#[server", "ServerFnError", "use_token", "use_tenant", "UiRouteContext"]) {
    assertNotContains(core, marker, `${corePath}: core DTO/view-model layer must remain Leptos/server-function free (${marker})`);
  }
  assertContains(core, "OutboxAdminBootstrap", `${corePath}: core must own bootstrap DTOs`);
  assertContains(core, "outbox_info_cards", `${corePath}: core must own view-model fallback policy`);
  assertContains(core, "unwrap_or_else(|| text.global_tenant_label.clone())", `${corePath}: tenant fallback must stay in core, not UI`);

  const fetchBody = rustFunctionBody(transport, "fetch_bootstrap");
  assertContains(fetchBody, "native_server_adapter::fetch_bootstrap_native()", `${transportPath}: facade must call the native server-function adapter`);
  assertContains(transport, "OutboxTransportError", `${transportPath}: facade must expose typed transport errors`);
  assertNotContains(transport, "#[server", `${transportPath}: server functions belong in native_server_adapter.rs`);
  assertNotContains(transport, "outbox_bootstrap_native().await", `${transportPath}: facade must not call raw generated server function directly`);

  assertContains(native, "#[server(prefix = \"/api/fn\", endpoint = \"outbox/bootstrap\")]", `${nativePath}: native bootstrap endpoint drift`);
  assertContains(native, "async fn outbox_bootstrap_native", `${nativePath}: generated server function must remain private to transport adapter`);
  assertContains(native, "pub async fn fetch_bootstrap_native", `${nativePath}: native adapter must expose a facade for transport/mod.rs`);

  const uiForbidden = [
    "#[server",
    "ServerFnError",
    "outbox_bootstrap_native",
    "fetch_bootstrap_native",
    "native_server_adapter",
    "leptos_axum::extract",
    "AppContext",
    "query_status_count",
    "query_scalar_i64",
    "GraphqlRequest",
    "execute_graphql",
    "/api/graphql",
  ];
  for (const marker of uiForbidden) {
    assertNotContains(ui, marker, `${uiPath}: UI adapter must not reach through the module-owned transport boundary (${marker})`);
  }
  assertContains(ui, "transport::fetch_bootstrap().await", `${uiPath}: UI adapter must use the transport facade`);
  assertContains(ui, "outbox_info_cards(&bootstrap, &text)", `${uiPath}: UI adapter must consume core-owned view models`);
  assertContains(ui, "UiRouteContext", `${uiPath}: UI locale must come from host-provided route context`);
  assertNotContains(ui, "navigator.language", `${uiPath}: UI must not invent package-local locale fallback chains`);

  assertContains(plan, "verify:outbox:admin-boundary", `${planPath}: local implementation plan must list the admin boundary verifier`);
  assertContains(registry, "verify:outbox:admin-boundary", `${registryPath}: central readiness board must list the admin boundary verifier`);
}

assertOutboxAdminBoundary();

if (failures.length > 0) {
  console.error("[verify-outbox-admin-boundary] Boundary verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("[verify-outbox-admin-boundary] Outbox admin core/transport/ui boundary is consistent");

#!/usr/bin/env node
// RusTok pricing admin FFA boundary guardrails.

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

const files = {
  lib: "crates/rustok-pricing/admin/src/lib.rs",
  core: "crates/rustok-pricing/admin/src/core/mod.rs",
  ui: "crates/rustok-pricing/admin/src/ui/leptos.rs",
  transport: "crates/rustok-pricing/admin/src/transport.rs",
  nativeServerAdapter: "crates/rustok-pricing/admin/src/transport/native_server_adapter.rs",
  legacyApi: "crates/rustok-pricing/admin/src/api.rs",
  implementationPlan: "crates/rustok-pricing/docs/implementation-plan.md",
  registry: "docs/modules/registry.md",
  packageJson: "package.json",
};

for (const [name, filePath] of Object.entries(files)) {
  if (name === "legacyApi") continue;
  assertExists(filePath, `${filePath}: expected pricing admin FFA boundary file`);
}
if (existsSync(repoPath(files.legacyApi))) {
  fail(`${files.legacyApi}: pricing admin legacy api.rs must stay removed; transport adapters own raw operations`);
}

const lib = readRepo(files.lib);
const core = readRepo(files.core);
const ui = readRepo(files.ui);
const transport = readRepo(files.transport);
const nativeServerAdapter = readRepo(files.nativeServerAdapter);
const implementationPlan = readRepo(files.implementationPlan);
const registry = readRepo(files.registry);
const packageJson = readRepo(files.packageJson);

assertNotContains(lib, "mod api;", `${files.lib}: crate root must not wire legacy api adapter`);
assertContains(lib, "mod core;", `${files.lib}: crate root must wire core`);
assertContains(lib, "mod transport;", `${files.lib}: crate root must wire transport facade`);
assertContains(lib, "mod ui;", `${files.lib}: crate root must wire UI adapter`);

for (const marker of ["leptos::", "leptos_", "#[component]", "#[server", "LocalResource", "WriteSignal", "web_sys::"]) {
  assertNotContains(core, marker, `${files.core}: core must stay Leptos/server-function free (${marker})`);
}
assertContains(ui, "use crate::core", `${files.ui}: UI must consume core layer`);
assertContains(ui, "use crate::transport", `${files.ui}: UI must consume transport facade`);
for (const marker of ["crate::api", /(^|[^A-Za-z0-9_])api::/, "#[server"]) {
  assertNotContains(ui, marker, `${files.ui}: UI adapter must not call raw transport or server functions (${marker})`);
}

assertContains(transport, "mod native_server_adapter;", `${files.transport}: transport must wire native server adapter`);
assertContains(transport, "native_server_adapter::", `${files.transport}: transport facade must delegate through adapter`);
assertNotContains(transport, "crate::api", `${files.transport}: transport facade must not import legacy api module`);
assertNotContains(transport, "#[server", `${files.transport}: transport facade must not own server functions`);
assertContains(nativeServerAdapter, "pub enum ApiError", `${files.nativeServerAdapter}: adapter must own shared ApiError envelope`);
assertContains(nativeServerAdapter, "#[server", `${files.nativeServerAdapter}: native server adapter must keep server functions`);
assertNotContains(nativeServerAdapter, "GraphqlRequest", `${files.nativeServerAdapter}: native adapter must not execute the parallel GraphQL contract`);

assertContains(implementationPlan, "verify-pricing-admin-boundary.mjs", `${files.implementationPlan}: local plan must mention pricing admin guardrail`);
assertContains(registry, "verify-pricing-admin-boundary.mjs", `${files.registry}: central readiness board must mention pricing admin guardrail`);
assertContains(packageJson, "verify:pricing:admin-boundary", `${files.packageJson}: package scripts must expose pricing admin boundary verification`);
assertContains(packageJson, "test:verify:pricing:admin-boundary", `${files.packageJson}: package scripts must expose pricing admin fixture tests`);
assertContains(packageJson, "npm run verify:pricing:admin-boundary", `${files.packageJson}: aggregate FFA verification must include pricing admin guardrail`);
assertContains(packageJson, "npm run test:verify:pricing:admin-boundary", `${files.packageJson}: aggregate FFA fixture tests must include pricing admin fixtures`);

if (failures.length > 0) {
  console.error("pricing admin boundary verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("pricing admin boundary verification passed");

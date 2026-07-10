#!/usr/bin/env node
// RusTok pricing storefront FFA boundary guardrails.

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
  lib: "crates/rustok-pricing/storefront/src/lib.rs",
  core: "crates/rustok-pricing/storefront/src/core.rs",
  ui: "crates/rustok-pricing/storefront/src/ui/leptos.rs",
  transport: "crates/rustok-pricing/storefront/src/transport/mod.rs",
  cargo: "crates/rustok-pricing/storefront/Cargo.toml",
  legacyApi: "crates/rustok-pricing/storefront/src/api.rs",
  graphqlAdapter: "crates/rustok-pricing/storefront/src/transport/graphql_adapter.rs",
  nativeServerAdapter: "crates/rustok-pricing/storefront/src/transport/native_server_adapter.rs",
  implementationPlan: "crates/rustok-pricing/docs/implementation-plan.md",
  registry: "docs/modules/registry.md",
  packageJson: "package.json",
};

for (const [name, filePath] of Object.entries(files)) {
  if (name === "legacyApi") continue;
  assertExists(filePath, `${filePath}: expected pricing storefront FFA boundary file`);
}
if (existsSync(repoPath(files.legacyApi))) {
  fail(`${files.legacyApi}: pricing storefront legacy api.rs must stay removed; transport adapters own raw operations`);
}

const lib = readRepo(files.lib);
const core = readRepo(files.core);
const ui = readRepo(files.ui);
const transport = readRepo(files.transport);
const cargo = readRepo(files.cargo);
const graphqlAdapter = readRepo(files.graphqlAdapter);
const nativeServerAdapter = readRepo(files.nativeServerAdapter);
const implementationPlan = readRepo(files.implementationPlan);
const registry = readRepo(files.registry);
const packageJson = readRepo(files.packageJson);

assertNotContains(lib, "mod api;", `${files.lib}: crate root must not wire legacy api adapter`);
assertContains(lib, "mod core;", `${files.lib}: crate root must wire core`);
assertContains(lib, "mod transport;", `${files.lib}: crate root must wire transport facade`);
assertContains(lib, "pub use ui::leptos::PricingView;", `${files.lib}: crate root must re-export PricingView`);

for (const marker of ["leptos::", "leptos_", "#[component]", "#[server", "LocalResource", "WriteSignal", "web_sys::"]) {
  assertNotContains(core, marker, `${files.core}: core must stay Leptos/server-function free (${marker})`);
}
assertContains(ui, "use crate::core", `${files.ui}: UI must consume core layer`);
assertContains(ui, "use crate::transport", `${files.ui}: UI must consume transport facade`);
for (const marker of ["crate::api", /(^|[^A-Za-z0-9_])api::/, "#[server", "PricingService"]) {
  assertNotContains(ui, marker, `${files.ui}: UI adapter must not call raw transport or services (${marker})`);
}

assertContains(transport, "fetch_storefront_pricing", `${files.transport}: transport facade must expose fetch_storefront_pricing`);
assertContains(transport, "mod graphql_adapter;", `${files.transport}: transport must wire GraphQL adapter`);
assertContains(transport, "mod native_server_adapter;", `${files.transport}: transport must wire native server adapter`);
assertNotContains(transport, "crate::api", `${files.transport}: transport facade must not import legacy api module`);
assertContains(graphqlAdapter, "GraphqlRequest", `${files.graphqlAdapter}: GraphQL adapter must delegate to the parallel GraphQL path`);
assertContains(nativeServerAdapter, "#[server", `${files.nativeServerAdapter}: native server adapter must keep server functions`);
assertNotContains(nativeServerAdapter, "GraphqlRequest", `${files.nativeServerAdapter}: native adapter must not execute the parallel GraphQL contract`);
assertContains(nativeServerAdapter, "expect_context::<HostRuntimeContext>()", `${files.nativeServerAdapter}: native adapter must use the host runtime context`);
assertContains(nativeServerAdapter, "shared_get::<TransactionalEventBus>()", `${files.nativeServerAdapter}: native adapter must receive the event bus through the host runtime context`);
assertContains(nativeServerAdapter, "runtime_ctx.db_clone()", `${files.nativeServerAdapter}: native adapter must receive DB through the host runtime context`);
assertNotContains(nativeServerAdapter, "loco_rs", `${files.nativeServerAdapter}: native adapter must not depend on Loco AppContext`);
assertNotContains(nativeServerAdapter, "rustok_outbox::loco", `${files.nativeServerAdapter}: native adapter must not use the outbox Loco adapter`);
assertNotContains(cargo, "loco-rs", `${files.cargo}: pricing storefront package must not depend on Loco`);
assertNotContains(cargo, "loco-adapter", `${files.cargo}: pricing storefront package must not enable the outbox Loco adapter`);

assertContains(implementationPlan, "verify-pricing-storefront-boundary.mjs", `${files.implementationPlan}: local plan must mention pricing storefront guardrail`);
assertContains(registry, "verify-pricing-storefront-boundary.mjs", `${files.registry}: central readiness board must mention pricing storefront guardrail`);
assertContains(packageJson, "verify:pricing:storefront-boundary", `${files.packageJson}: package scripts must expose pricing storefront boundary verification`);
assertContains(packageJson, "test:verify:pricing:storefront-boundary", `${files.packageJson}: package scripts must expose pricing storefront fixture tests`);
assertContains(packageJson, "npm run verify:pricing:storefront-boundary", `${files.packageJson}: aggregate FFA verification must include pricing storefront guardrail`);
assertContains(packageJson, "npm run test:verify:pricing:storefront-boundary", `${files.packageJson}: aggregate FFA fixture tests must include pricing storefront fixtures`);

if (failures.length > 0) {
  console.error("pricing storefront boundary verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("pricing storefront boundary verification passed");

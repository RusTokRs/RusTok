#!/usr/bin/env node
// RusTok order admin FFA boundary guardrails.
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

const libPath = "crates/rustok-order/admin/src/lib.rs";
const coreModPath = "crates/rustok-order/admin/src/core/mod.rs";
const coreRequestsPath = "crates/rustok-order/admin/src/core/requests.rs";
const coreCommandsPath = "crates/rustok-order/admin/src/core/commands.rs";
const coreDetailFormPath = "crates/rustok-order/admin/src/core/detail_form.rs";
const corePresentationPath = "crates/rustok-order/admin/src/core/presentation.rs";
const uiPath = "crates/rustok-order/admin/src/ui/leptos.rs";
const helpersPath = "crates/rustok-order/admin/src/helpers.rs";
const transportPath = "crates/rustok-order/admin/src/transport/mod.rs";
const graphqlAdapterPath = "crates/rustok-order/admin/src/transport/graphql_adapter.rs";
const implementationPlanPath = "crates/rustok-order/docs/implementation-plan.md";
const registryPath = "docs/modules/registry.md";

for (const filePath of [
  libPath,
  coreModPath,
  coreRequestsPath,
  coreCommandsPath,
  coreDetailFormPath,
  corePresentationPath,
  uiPath,
  helpersPath,
  transportPath,
  graphqlAdapterPath,
  implementationPlanPath,
  registryPath,
]) {
  assertExists(filePath, `${filePath}: expected order admin FFA boundary file`);
}

const lib = readRepo(libPath);
const core = [coreModPath, coreRequestsPath, coreCommandsPath, coreDetailFormPath, corePresentationPath]
  .map((filePath) => readRepo(filePath))
  .join("\n");
const ui = readRepo(uiPath);
const transport = readRepo(transportPath);
const graphqlAdapter = readRepo(graphqlAdapterPath);
const implementationPlan = readRepo(implementationPlanPath);
const registry = readRepo(registryPath);

assertNotContains(lib, "mod api;", `${libPath}: crate root must not wire raw API adapter at root after transport split`);
assertContains(lib, "mod core;", `${libPath}: crate root must wire core directory`);
assertContains(readRepo(coreModPath), "mod commands;", `${coreModPath}: core directory must split command policy`);
assertContains(readRepo(coreModPath), "mod detail_form;", `${coreModPath}: core directory must split detail form-state policy`);
assertContains(readRepo(coreModPath), "mod presentation;", `${coreModPath}: core directory must split presentation policy`);
assertContains(readRepo(coreModPath), "mod requests;", `${coreModPath}: core directory must split request policy`);
assertContains(lib, "mod transport;", `${libPath}: crate root must wire transport facade`);
assertContains(lib, "mod ui;", `${libPath}: crate root must wire UI adapters`);
assertContains(lib, "pub use ui::OrderAdmin;", `${libPath}: crate root must re-export OrderAdmin`);
for (const marker of [/pub async fn fetch_/, /pub async fn mark_/, /pub async fn ship_/, /pub async fn deliver_/, /pub async fn cancel_/]) {
  assertNotContains(lib, marker, `${libPath}: crate root must not expose public transport passthroughs (${marker})`);
}

for (const marker of ["leptos::", "leptos_", "#[component]", "#[server", "LocalResource", "WriteSignal", "web_sys::"]) {
  assertNotContains(core, marker, `crates/rustok-order/admin/src/core/: core must stay Leptos/server-function free (${marker})`);
}
for (const marker of [
  "OrderListRequest",
  "order_list_request",
  "OrderMarkPaidCommand",
  "OrderShipCommand",
  "OrderDeliverCommand",
  "OrderCancelCommand",
  "prepare_mark_paid_command",
  "prepare_ship_order_command",
  "prepare_deliver_order_command",
  "prepare_cancel_order_command",
  "localized_order_status",
  "order_status_badge",
  "summarize_order_lines",
  "format_order_caption",
  "summarize_order_header",
  "summarize_order_timeline",
  "action_hint",
  "text_or_dash",
  "OrderAdminDetailFormState",
  "order_detail_form_state",
]) {
  assertContains(core, marker, `crates/rustok-order/admin/src/core/: expected core-owned FFA helper ${marker}`);
}

assertContains(ui, "use crate::core::{", `${uiPath}: Leptos adapter must import core-owned helpers`);
assertContains(ui, "use crate::transport;", `${uiPath}: Leptos adapter must call the module-owned transport facade`);
assertContains(ui, "action_hint", `${uiPath}: UI must consume core-owned presentation helpers`);
assertContains(ui, "use crate::helpers::{apply_order_detail, clear_order_detail, handle_action_result};", `${uiPath}: Leptos-specific helpers should stay limited to signal/side-effect helpers`);
assertContains(readRepo("crates/rustok-order/admin/src/helpers.rs"), "order_detail_form_state", `${helpersPath}: signal helpers must consume core-owned detail form-state mapping`);
for (const marker of [
  "order_list_request",
  "prepare_mark_paid_command",
  "prepare_ship_order_command",
  "prepare_deliver_order_command",
  "prepare_cancel_order_command",
  "localized_order_status",
  "order_status_badge",
  "summarize_order_lines",
  "format_order_caption",
  "summarize_order_header",
  "summarize_order_timeline",
  "action_hint",
  "text_or_dash",
]) {
  assertContains(ui, marker, `${uiPath}: UI must use core-owned order action helper ${marker}`);
}
for (const marker of ["crate::api", /(^|[^A-Za-z0-9_])api::/, "#[server", "OrderService", "PaymentService", "FulfillmentService"]) {
  assertNotContains(ui, marker, `${uiPath}: UI adapter must not call raw transport or services (${marker})`);
}

for (const marker of [
  "fetch_bootstrap",
  "fetch_orders",
  "fetch_order_detail",
  "mark_order_paid",
  "ship_order",
  "deliver_order",
  "cancel_order",
]) {
  assertContains(transport, marker, `${transportPath}: transport facade must expose ${marker}`);
}
assertContains(transport, "mod graphql_adapter;", `${transportPath}: transport facade must own the GraphQL adapter module`);
assertContains(transport, "graphql_adapter::", `${transportPath}: transport facade must delegate through the GraphQL adapter`);
assertNotContains(transport, "#[server", `${transportPath}: server/native endpoints must not live in the order admin transport facade`);
assertContains(graphqlAdapter, "GraphqlRequest", `${graphqlAdapterPath}: order admin GraphQL adapter must keep the GraphQL transport contract`);
assertContains(graphqlAdapter, "execute_graphql", `${graphqlAdapterPath}: GraphQL adapter must own execute_graphql calls`);

assertContains(implementationPlan, "verify-order-admin-boundary.mjs", `${implementationPlanPath}: local plan must mention the order fast boundary guardrail`);
assertContains(registry, "verify-order-admin-boundary.mjs", `${registryPath}: central readiness board must mention the order fast boundary guardrail`);

if (failures.length > 0) {
  console.error("order admin boundary verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("order admin boundary verification passed");

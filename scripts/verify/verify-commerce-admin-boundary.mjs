#!/usr/bin/env node
// RusTok commerce admin FFA boundary guardrails.

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
  lib: "crates/rustok-commerce/admin/src/lib.rs",
  core: "crates/rustok-commerce/admin/src/core/mod.rs",
  ui: "crates/rustok-commerce/admin/src/ui/leptos.rs",
  transport: "crates/rustok-commerce/admin/src/transport/mod.rs",
  shippingProfile: "crates/rustok-commerce/admin/src/transport/shipping_profile.rs",
  promotion: "crates/rustok-commerce/admin/src/transport/promotion.rs",
  orderChange: "crates/rustok-commerce/admin/src/transport/order_change.rs",
  nativeAdapter: "crates/rustok-commerce/admin/src/transport/native_server_adapter.rs",
  commerceRoot: "crates/rustok-commerce/src/lib.rs",
  providerOperations: "crates/rustok-commerce/src/graphql/mutations/provider_operations.rs",
  graphqlFulfillment: "crates/rustok-commerce/src/graphql/mutations/fulfillment.rs",
  graphqlRuntime: "crates/rustok-commerce/src/graphql_runtime.rs",
  fulfillmentFacade: "crates/rustok-commerce/src/services/fulfillment_orchestration_facade.rs",
  fulfillmentGuard: "apps/server/tests/commerce_fulfillment_transport_guard.rs",
  adminChanges: "crates/rustok-commerce/src/controllers/admin/changes.rs",
  orderChangeOrchestration:
    "crates/rustok-commerce/src/services/order_change_orchestration.rs",
  orderChangeGuard: "apps/server/tests/commerce_order_change_transport_guard.rs",
  legacyApi: "crates/rustok-commerce/admin/src/api.rs",
  implementationPlan: "crates/rustok-commerce/docs/implementation-plan.md",
  registry: "docs/modules/registry.md",
  packageJson: "package.json",
};

for (const [name, filePath] of Object.entries(files)) {
  if (name === "legacyApi") continue;
  assertExists(filePath, `${filePath}: expected commerce admin FFA boundary file`);
}
if (existsSync(repoPath(files.legacyApi))) {
  fail(`${files.legacyApi}: commerce admin legacy api.rs must stay removed; transport/native_server_adapter.rs owns native operations`);
}

const lib = readRepo(files.lib);
const core = readRepo(files.core);
const ui = readRepo(files.ui);
const transport = readRepo(files.transport);
const shippingProfile = readRepo(files.shippingProfile);
const promotion = readRepo(files.promotion);
const orderChange = readRepo(files.orderChange);
const nativeAdapter = readRepo(files.nativeAdapter);
const commerceRoot = readRepo(files.commerceRoot);
const providerOperations = readRepo(files.providerOperations);
const graphqlFulfillment = readRepo(files.graphqlFulfillment);
const graphqlRuntime = readRepo(files.graphqlRuntime);
const fulfillmentFacade = readRepo(files.fulfillmentFacade);
const fulfillmentGuard = readRepo(files.fulfillmentGuard);
const adminChanges = readRepo(files.adminChanges);
const orderChangeOrchestration = readRepo(files.orderChangeOrchestration);
const orderChangeGuard = readRepo(files.orderChangeGuard);
const implementationPlan = readRepo(files.implementationPlan);
const registry = readRepo(files.registry);
const packageJson = readRepo(files.packageJson);

assertNotContains(lib, "mod api;", `${files.lib}: crate root must not wire legacy api module`);
assertContains(lib, "mod core;", `${files.lib}: crate root must wire core`);
assertContains(lib, "mod transport;", `${files.lib}: crate root must wire transport`);
assertContains(lib, "mod ui;", `${files.lib}: crate root must wire UI`);

for (const marker of ["leptos::", "leptos_", "#[component]", "#[server", "LocalResource", "WriteSignal", "web_sys::"]) {
  assertNotContains(core, marker, `${files.core}: core must stay Leptos/server-function free (${marker})`);
}
assertContains(ui, /use crate::(?:\{[^}]*transport[^}]*\}|transport)/, `${files.ui}: UI must consume transport facade`);
for (const marker of ["crate::api", /(^|[^A-Za-z0-9_])api::/, "#[server"]) {
  assertNotContains(ui, marker, `${files.ui}: UI adapter must not call raw transport or server functions (${marker})`);
}

assertContains(transport, "mod native_server_adapter;", `${files.transport}: transport must wire native server adapter inside transport boundary`);
for (const [source, filePath] of [
  [shippingProfile, files.shippingProfile],
  [promotion, files.promotion],
  [orderChange, files.orderChange],
]) {
  assertContains(source, "native_server_adapter", `${filePath}: transport slice must consume native server adapter error contract or operations`);
  assertNotContains(source, "crate::api", `${filePath}: transport slice must not import legacy api module`);
}
assertContains(nativeAdapter, "pub enum ApiError", `${files.nativeAdapter}: native server adapter must own shared ApiError envelope`);
assertContains(nativeAdapter, "#[server", `${files.nativeAdapter}: native server adapter must keep native server-function endpoints`);

for (const marker of [
  "pub use graphql::{CommerceMutation, CommerceQuery}",
  "pub use state_machine::{",
]) {
  assertNotContains(commerceRoot, marker, `${files.commerceRoot}: root GraphQL/state-machine aliases must stay removed (${marker})`);
}
assertContains(commerceRoot, "pub mod graphql;", `${files.commerceRoot}: GraphQL module path must remain explicit`);
assertContains(commerceRoot, "pub mod state_machine;", `${files.commerceRoot}: state-machine module path must remain explicit`);

assertNotContains(
  providerOperations,
  "use rustok_fulfillment::FulfillmentService;",
  `${files.providerOperations}: GraphQL transport must not import the fulfillment owner service`,
);
assertNotContains(
  providerOperations,
  "FulfillmentService::new(",
  `${files.providerOperations}: GraphQL fulfillment mutations must use commerce orchestration`,
);
for (const operation of [
  ".create_manual_fulfillment(",
  ".ship_fulfillment(",
  ".deliver_fulfillment(",
  ".reopen_fulfillment(",
  ".reship_fulfillment(",
  ".cancel_fulfillment(",
]) {
  assertContains(
    providerOperations,
    operation,
    `${files.providerOperations}: missing fulfillment orchestration call ${operation}`,
  );
}
for (const method of [
  "pub async fn deliver_fulfillment(",
  "pub async fn reopen_fulfillment(",
]) {
  assertContains(
    fulfillmentFacade,
    method,
    `${files.fulfillmentFacade}: missing transport-safe fulfillment facade method ${method}`,
  );
}
assertContains(
  fulfillmentGuard,
  "graphql_fulfillment_mutations_use_commerce_orchestration",
  `${files.fulfillmentGuard}: fulfillment transport source guard is missing`,
);

assertContains(
  adminChanges,
  "OrderChangeOrchestrationService::new(",
  `${files.adminChanges}: REST order-change apply must use commerce orchestration`,
);
assertContains(
  adminChanges,
  ".apply_order_change(tenant.id, id, input.difference_refund, input.metadata)",
  `${files.adminChanges}: REST order-change apply must pass the complete command`,
);
assertNotContains(
  adminChanges,
  "match order_change.change_type.as_str()",
  `${files.adminChanges}: REST transport must not dispatch order-change domain types`,
);

assertContains(
  graphqlFulfillment,
  "order_change_orchestration_from_context(",
  `${files.graphqlFulfillment}: GraphQL order-change apply must use composed commerce orchestration`,
);
assertContains(
  graphqlFulfillment,
  ".apply_order_change(tenant_id, id, difference_refund, metadata)",
  `${files.graphqlFulfillment}: GraphQL order-change apply must pass the complete command`,
);
for (const marker of [
  "match order_change.change_type.as_str()",
  ".apply_exchange_order_change(",
  ".apply_claim_order_change(",
]) {
  assertNotContains(
    graphqlFulfillment,
    marker,
    `${files.graphqlFulfillment}: GraphQL transport must not own order-change dispatch (${marker})`,
  );
}
assertContains(
  graphqlRuntime,
  "pub(crate) fn order_change_orchestration_from_context(",
  `${files.graphqlRuntime}: GraphQL runtime must compose order-change orchestration`,
);

assertContains(
  orderChangeOrchestration,
  "match order_change.change_type.as_str()",
  `${files.orderChangeOrchestration}: orchestration must own order-change type dispatch`,
);
for (const operation of [
  ".apply_exchange_order_change(",
  ".apply_claim_order_change(",
  ".apply_order_change(",
]) {
  assertContains(
    orderChangeOrchestration,
    operation,
    `${files.orderChangeOrchestration}: missing order-change orchestration call ${operation}`,
  );
}
assertContains(
  orderChangeGuard,
  "order_change_application_uses_commerce_orchestration",
  `${files.orderChangeGuard}: order-change transport source guard is missing`,
);

assertContains(implementationPlan, "verify-commerce-admin-boundary.mjs", `${files.implementationPlan}: local plan must mention commerce admin guardrail`);
assertContains(implementationPlan, "admin/src/transport/native_server_adapter.rs", `${files.implementationPlan}: local plan must document commerce admin native adapter location`);
assertContains(implementationPlan, "root GraphQL and state-machine aliases", `${files.implementationPlan}: local plan must document removed root GraphQL/state-machine aliases`);
assertContains(registry, "verify-commerce-admin-boundary.mjs", `${files.registry}: central readiness board must mention commerce admin guardrail`);
assertContains(registry, "root GraphQL/state-machine aliases", `${files.registry}: central registry must document removed root GraphQL/state-machine aliases`);
assertContains(packageJson, "verify:commerce:admin-boundary", `${files.packageJson}: package scripts must expose commerce admin boundary verification`);
assertContains(packageJson, "test:verify:commerce:admin-boundary", `${files.packageJson}: package scripts must expose commerce admin fixture tests`);
assertContains(packageJson, "npm run verify:commerce:admin-boundary", `${files.packageJson}: aggregate FFA verification must include commerce admin guardrail`);
assertContains(packageJson, "npm run test:verify:commerce:admin-boundary", `${files.packageJson}: aggregate FFA fixture tests must include commerce admin fixtures`);

if (failures.length > 0) {
  console.error("commerce admin boundary verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("commerce admin boundary verification passed");
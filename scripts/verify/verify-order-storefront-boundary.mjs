#!/usr/bin/env node
// RusTok order storefront FFA boundary guardrails.
// Fast source-level checks for order-owned checkout result/action UI and request ownership.

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

const libPath = "crates/rustok-order/storefront/src/lib.rs";
const corePath = "crates/rustok-order/storefront/src/core.rs";
const transportPath = "crates/rustok-order/storefront/src/transport.rs";
const uiPath = "crates/rustok-order/storefront/src/ui/leptos.rs";
const commerceUiPath = "crates/rustok-commerce/storefront/src/ui/leptos/mod.rs";
const commerceRequestsPath = "crates/rustok-commerce/storefront/src/core/requests.rs";
const planPath = "crates/rustok-order/docs/implementation-plan.md";
const registryPath = "docs/modules/registry.md";
const packagePath = "package.json";

for (const filePath of [libPath, corePath, transportPath, uiPath, commerceUiPath, commerceRequestsPath, planPath, registryPath, packagePath]) {
  assertExists(filePath, `${filePath}: expected order storefront FFA file`);
}

const lib = readRepo(libPath);
const core = readRepo(corePath);
const transport = readRepo(transportPath);
const ui = readRepo(uiPath);
const commerceUi = readRepo(commerceUiPath);
const commerceRequests = readRepo(commerceRequestsPath);
const plan = readRepo(planPath);
const registry = readRepo(registryPath);
const packageJson = readRepo(packagePath);

for (const marker of ["pub mod core;", "pub mod transport;", "OrderCheckoutCompleteButton", "OrderCheckoutResultCard"]) {
  assertContains(lib, marker, `${libPath}: expected storefront public surface marker ${marker}`);
}

for (const marker of [
  "OrderCheckoutResultData",
  "OrderCheckoutResultViewModel",
  "build_order_checkout_result_view_model",
  "OrderCheckoutActionLabels",
  "order_checkout_action_label",
]) {
  assertContains(core, marker, `${corePath}: expected core-owned order presentation marker ${marker}`);
}
for (const marker of ["leptos::", "#[component]", "#[server", "GraphqlRequest", "web_sys::"]) {
  assertNotContains(core, marker, `${corePath}: core must stay UI/transport free (${marker})`);
}

for (const marker of ["CompleteCheckoutRequest", "build_complete_checkout_request", "normalize_required"]) {
  assertContains(transport, marker, `${transportPath}: expected transport-owned request marker ${marker}`);
}
for (const marker of ["leptos::", "#[component]", "#[server", "GraphqlRequest", "web_sys::"]) {
  assertNotContains(transport, marker, `${transportPath}: transport facade must stay framework/native-endpoint free (${marker})`);
}

for (const marker of [
  "OrderCheckoutCompleteButton",
  "OrderCheckoutResultCard",
  "CompleteCheckoutRequest",
  "build_complete_checkout_request",
  "on_complete_checkout: Callback<CompleteCheckoutRequest>",
]) {
  assertContains(ui, marker, `${uiPath}: expected order-owned UI/request marker ${marker}`);
}
for (const marker of ["crate::api", "rustok_commerce::", "GraphqlRequest", "#[server"]) {
  assertNotContains(ui, marker, `${uiPath}: UI adapter must not call commerce/raw transport directly (${marker})`);
}

assertContains(commerceUi, "OrderCheckoutCompleteButton", `${commerceUiPath}: commerce host must render order-owned complete-checkout UI`);
assertContains(commerceUi, "Callback::new(move |request: CompleteCheckoutRequest|", `${commerceUiPath}: commerce callback must accept order-owned request DTO`);
assertNotContains(commerceUi, "build_checkout_completion_command_request", `${commerceUiPath}: commerce UI must not rebuild order requests from raw cart ids`);
assertContains(commerceRequests, "pub type CheckoutCompletionCommandRequest = CompleteCheckoutRequest", `${commerceRequestsPath}: commerce transport may keep transitional alias to owner request`);
assertNotContains(commerceRequests, "build_complete_checkout_request", `${commerceRequestsPath}: commerce core must not wrap order-owned request construction`);
assertNotContains(commerceRequests, "build_checkout_completion_command_request", `${commerceRequestsPath}: commerce core must not expose an order request builder after owner UI handoff`);
assertContains(plan, "verify-order-storefront-boundary.mjs", `${planPath}: local plan must mention storefront boundary guardrail`);
assertContains(registry, "verify-order-storefront-boundary.mjs", `${registryPath}: central registry must mention storefront boundary guardrail`);
assertContains(packageJson, "verify:order:storefront-boundary", `${packagePath}: expected order storefront boundary script`);
assertContains(packageJson, "npm run verify:order:storefront-boundary", `${packagePath}: aggregate FFA migration verification must include storefront order boundary`);

if (failures.length > 0) {
  console.error("order storefront boundary verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("order storefront boundary verification passed");

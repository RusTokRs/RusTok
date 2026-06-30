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
const graphqlPath = "crates/rustok-order/storefront/src/transport/graphql_adapter.rs";
const nativeRawPath = "crates/rustok-order/storefront/src/transport/native_server_adapter/raw_adapter.rs";
const uiPath = "crates/rustok-order/storefront/src/ui/leptos.rs";
const i18nPath = "crates/rustok-order/storefront/src/i18n.rs";
const manifestPath = "crates/rustok-order/rustok-module.toml";
const commerceUiPath = "crates/rustok-commerce/storefront/src/ui/leptos/mod.rs";
const commerceRequestsPath = "crates/rustok-commerce/storefront/src/core/requests.rs";
const planPath = "crates/rustok-order/docs/implementation-plan.md";
const registryPath = "docs/modules/registry.md";
const packagePath = "package.json";

for (const filePath of [libPath, corePath, transportPath, graphqlPath, nativeRawPath, uiPath, i18nPath, manifestPath, commerceUiPath, commerceRequestsPath, planPath, registryPath, packagePath]) {
  assertExists(filePath, `${filePath}: expected order storefront FFA file`);
}

const lib = readRepo(libPath);
const core = readRepo(corePath);
const transport = readRepo(transportPath);
const graphql = readRepo(graphqlPath);
const nativeRaw = readRepo(nativeRawPath);
const ui = readRepo(uiPath);
const i18n = readRepo(i18nPath);
const manifest = readRepo(manifestPath);
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

for (const marker of ["CompleteCheckoutRequest", "CheckoutCompletion", "build_complete_checkout_request", "complete_checkout", "mod graphql_adapter;", "normalize_required"]) {
  assertContains(transport, marker, `${transportPath}: expected transport-owned request marker ${marker}`);
}
assertContains(transport, "mod native_server_adapter;", `${transportPath}: order transport facade must wire native server adapter`);
for (const marker of ["leptos::", "#[component]", "#[server", "GraphqlRequest", "web_sys::"]) {
  assertNotContains(transport, marker, `${transportPath}: transport facade must stay framework/native-endpoint free (${marker})`);
}
for (const marker of ["COMPLETE_STOREFRONT_CHECKOUT_MUTATION", "GraphqlRequest::new", "CheckoutAdjustment"]) {
  assertContains(graphql, marker, `${graphqlPath}: order must own GraphQL completion marker ${marker}`);
}
assertNotContains(graphql, "rustok_commerce::", `${graphqlPath}: order GraphQL adapter must not depend on commerce storefront internals`);
assertContains(nativeRaw, "#[server", `${nativeRawPath}: order native adapter must own a server-function endpoint shell`);
assertContains(nativeRaw, "endpoint = \"order/complete-checkout\"", `${nativeRawPath}: order native adapter must expose the owner endpoint path`);
assertContains(nativeRaw, "rustok_commerce::storefront_checkout_runtime", `${nativeRawPath}: order native adapter must call the explicit commerce checkout runtime API`);

for (const marker of [
  "OrderView",
  "use_context::<UiRouteContext>()",
  "crate::i18n::t",
  "OrderCheckoutCompleteButton",
  "OrderCheckoutResultCard",
  "CompleteCheckoutRequest",
  "build_complete_checkout_request",
  "on_complete_checkout: Callback<CompleteCheckoutRequest>",
]) {
  assertContains(ui, marker, `${uiPath}: expected order-owned UI/request marker ${marker}`);
}
for (const marker of ["include_str!(\"../locales/en.json\")", "include_str!(\"../locales/ru.json\")", "resolve_ui_message_or_fallback"]) {
  assertContains(i18n, marker, `${i18nPath}: expected host-locale catalog marker ${marker}`);
}
for (const marker of ["slot = \"checkout_result_handoff\"", "[provides.storefront_ui.i18n]", "leptos_locales_path = \"storefront/locales\""]) {
  assertContains(manifest, marker, `${manifestPath}: expected locale-aware storefront manifest marker ${marker}`);
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

#!/usr/bin/env node
// RusTok commerce storefront transport handoff guardrails.
// Fast source-level checks that aggregate checkout keeps owner DTOs and only falls back
// to GraphQL when native server functions are unavailable, not for validation/domain errors.

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

const requestsPath = "crates/rustok-commerce/storefront/src/core/requests.rs";
const libPath = "crates/rustok-commerce/storefront/src/lib.rs";
const transportPath = "crates/rustok-commerce/storefront/src/transport/mod.rs";
const nativePath = "crates/rustok-commerce/storefront/src/transport/native_server_adapter.rs";
const graphqlPath = "crates/rustok-commerce/storefront/src/transport/graphql_adapter.rs";
const rawPath = "crates/rustok-commerce/storefront/src/transport/raw_adapter.rs";
const legacyApiPath = "crates/rustok-commerce/storefront/src/api.rs";
const paymentTransportPath = "crates/rustok-payment/storefront/src/transport.rs";
const orderTransportPath = "crates/rustok-order/storefront/src/transport.rs";
const fulfillmentTransportPath = "crates/rustok-fulfillment/storefront/src/transport.rs";
const commercePlanPath = "crates/rustok-commerce/docs/implementation-plan.md";
const paymentPlanPath = "crates/rustok-payment/docs/implementation-plan.md";
const orderPlanPath = "crates/rustok-order/docs/implementation-plan.md";
const fulfillmentPlanPath = "crates/rustok-fulfillment/docs/implementation-plan.md";
const registryPath = "docs/modules/registry.md";
const packagePath = "package.json";

for (const filePath of [requestsPath, libPath, transportPath, nativePath, graphqlPath, rawPath, paymentTransportPath, orderTransportPath, fulfillmentTransportPath, commercePlanPath, paymentPlanPath, orderPlanPath, fulfillmentPlanPath, registryPath, packagePath]) {
  assertExists(filePath, `${filePath}: expected storefront transport handoff file`);
}
if (existsSync(repoPath(legacyApiPath))) {
  fail(`${legacyApiPath}: commerce storefront legacy api.rs must stay removed; transport/raw_adapter.rs owns raw operations`);
}

const requests = readRepo(requestsPath);
const lib = readRepo(libPath);
const transport = readRepo(transportPath);
const nativeAdapter = readRepo(nativePath);
const graphqlAdapter = readRepo(graphqlPath);
const rawAdapter = readRepo(rawPath);
const paymentTransport = readRepo(paymentTransportPath);
const orderTransport = readRepo(orderTransportPath);
const fulfillmentTransport = readRepo(fulfillmentTransportPath);
const commercePlan = readRepo(commercePlanPath);
const paymentPlan = readRepo(paymentPlanPath);
const orderPlan = readRepo(orderPlanPath);
const fulfillmentPlan = readRepo(fulfillmentPlanPath);
const registry = readRepo(registryPath);
const packageJson = readRepo(packagePath);

for (const marker of [
  "pub type PaymentCollectionCommandRequest = PaymentCollectionCreateRequest",
  "pub type CheckoutCompletionCommandRequest = CompleteCheckoutRequest",
  "FulfillmentSelectShippingOptionRequest",
]) {
  assertContains(requests, marker, `${requestsPath}: aggregate checkout must consume owner request DTO marker ${marker}`);
}
for (const marker of [
  "build_payment_collection_command_request",
  "build_checkout_completion_command_request",
  "pub struct PaymentCollectionCommandRequest",
  "pub struct CheckoutCompletionCommandRequest",
]) {
  assertNotContains(requests, marker, `${requestsPath}: aggregate checkout must not recreate owner command DTOs (${marker})`);
}

for (const operation of [
  "fetch_storefront_commerce",
  "create_storefront_payment_collection",
  "select_storefront_shipping_option",
  "complete_storefront_checkout",
]) {
  assertContains(transport, `pub async fn ${operation}`, `${transportPath}: missing transport operation ${operation}`);
}
assertContains(transport, "Err(error) if should_fallback_to_graphql(&error)", `${transportPath}: native fallback must be explicitly gated`);
assertContains(transport, "Err(error) => Err(error)", `${transportPath}: native validation/domain errors must be returned without GraphQL fallback`);
assertContains(transport, "validation_and_graphql_errors_do_not_trigger_compatibility_fallback", `${transportPath}: fallback guardrail unit test marker missing`);
for (const marker of [
  "create_payment_collection_with_fallback",
  "PaymentCollectionTransportError",
  "complete_checkout_with_fallback",
  "CheckoutCompletionTransportError",
  "select_shipping_option_with_fallback",
  "ShippingSelectionTransportError",
]) {
  assertContains(transport, marker, `${transportPath}: aggregate checkout must delegate owner handoff fallback policy through ${marker}`);
}
for (const marker of [
  "Err(_) => graphql_adapter::create_storefront_payment_collection",
  "Err(_) => graphql_adapter::complete_storefront_checkout",
  "Err(_) => graphql_adapter::select_storefront_shipping_option",
  "Err(_) => graphql_adapter::fetch_storefront_commerce",
]) {
  assertNotContains(transport, marker, `${transportPath}: broad GraphQL fallback is forbidden for owner handoff paths (${marker})`);
}
assertNotContains(lib, "mod api;", `${libPath}: crate root must not wire legacy api module`);
assertContains(transport, "mod raw_adapter;", `${transportPath}: transport facade must wire raw adapter inside transport boundary`);
assertContains(transport, "use raw_adapter::ApiError;", `${transportPath}: transport facade must expose ApiError from raw adapter`);
assertNotContains(transport, "crate::api", `${transportPath}: transport facade must not delegate to legacy api module`);
assertNotContains(nativeAdapter, "crate::api", `${nativePath}: native adapter must not delegate to legacy api module`);
assertNotContains(graphqlAdapter, "crate::api", `${graphqlPath}: GraphQL adapter must not delegate to legacy api module`);
assertContains(rawAdapter, "#[server", `${rawPath}: raw adapter must keep native server-function endpoints`);
assertContains(rawAdapter, "GraphqlRequest", `${rawPath}: raw adapter must keep GraphQL fallback request contract until split further`);
assertContains(rawAdapter, "build_shipping_selection_updates", `${rawPath}: commerce raw adapter must consume fulfillment-owned shipping selection materialization`);
assertNotContains(rawAdapter, "build_shipping_selection_plan", `${rawPath}: commerce raw adapter must not own shipping selection planning`);
assertNotContains(rawAdapter, "fn shipping_selection_error_message", `${rawPath}: commerce raw adapter must not own fulfillment selection error text`);

for (const [ownerTransport, ownerPath, fallbackFn, errorType] of [
  [paymentTransport, paymentTransportPath, "create_payment_collection_with_fallback", "PaymentCollectionTransportError"],
  [orderTransport, orderTransportPath, "complete_checkout_with_fallback", "CheckoutCompletionTransportError"],
  [fulfillmentTransport, fulfillmentTransportPath, "select_shipping_option_with_fallback", "ShippingSelectionTransportError"],
]) {
  assertContains(ownerTransport, `pub enum ${errorType}`, `${ownerPath}: owner transport must expose typed fallback error ${errorType}`);
  assertContains(ownerTransport, `pub async fn ${fallbackFn}`, `${ownerPath}: owner transport must expose fallback facade ${fallbackFn}`);
  assertContains(ownerTransport, "Err(error) if error.should_fallback_to_graphql()", `${ownerPath}: owner fallback facade must be MissingServer-gated`);
  assertContains(ownerTransport, "Err(error) => Err(error)", `${ownerPath}: owner fallback facade must preserve validation/domain errors`);
}
assertContains(fulfillmentTransport, "build_shipping_selection_updates", `${fulfillmentTransportPath}: fulfillment transport must own shipping selection materialization for compatibility cutover`);

for (const [operation, requestType] of [
  ["create_storefront_payment_collection", "PaymentCollectionCommandRequest"],
  ["complete_storefront_checkout", "CheckoutCompletionCommandRequest"],
  ["select_storefront_shipping_option", "SelectShippingOptionRequest"],
]) {
  const signature = new RegExp(`${operation}\\s*\\(\\s*request:\\s*${requestType}`);
  assertContains(nativeAdapter, signature, `${nativePath}: native adapter must accept owner/alias request ${operation}(${requestType})`);
  assertContains(graphqlAdapter, signature, `${graphqlPath}: GraphQL adapter must accept owner/alias request ${operation}(${requestType})`);
}

assertContains(commercePlan, "verify-commerce-storefront-transport-handoff.mjs", `${commercePlanPath}: commerce plan must mention transport handoff guardrail`);
assertContains(commercePlan, "storefront/src/transport/raw_adapter.rs", `${commercePlanPath}: commerce plan must document storefront raw adapter location`);
assertContains(paymentPlan, "compatibility fallback is now MissingServer-only", `${paymentPlanPath}: payment plan must document narrowed fallback policy`);
assertContains(orderPlan, "compatibility fallback is now MissingServer-only", `${orderPlanPath}: order plan must document narrowed fallback policy`);
assertContains(fulfillmentPlan, "compatibility fallback is now MissingServer-only", `${fulfillmentPlanPath}: fulfillment plan must document narrowed fallback policy`);
assertContains(registry, "verify-commerce-storefront-transport-handoff.mjs", `${registryPath}: central registry must mention transport handoff guardrail`);
assertContains(registry, "storefront/src/transport/raw_adapter.rs", `${registryPath}: central registry must document commerce storefront raw adapter location`);
assertContains(packageJson, "verify:commerce:storefront-transport-handoff", `${packagePath}: expected transport handoff script`);
assertContains(packageJson, "npm run verify:commerce:storefront-transport-handoff", `${packagePath}: aggregate FFA migration verification must include transport handoff guardrail`);

if (failures.length > 0) {
  console.error("commerce storefront transport handoff verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("commerce storefront transport handoff verification passed");

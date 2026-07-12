#!/usr/bin/env node
// RusTok fulfillment storefront FFA boundary guardrails.
// Fast source-level checks for module-owned shipping selection UI/core ownership.

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

const libPath = "crates/rustok-fulfillment/storefront/src/lib.rs";
const modelPath = "crates/rustok-fulfillment/storefront/src/model.rs";
const corePath = "crates/rustok-fulfillment/storefront/src/core/mod.rs";
const uiPath = "crates/rustok-fulfillment/storefront/src/ui/leptos.rs";
const i18nPath = "crates/rustok-fulfillment/storefront/src/i18n.rs";
const manifestPath = "crates/rustok-fulfillment/rustok-module.toml";
const transportPath = "crates/rustok-fulfillment/storefront/src/transport.rs";
const graphqlPath = "crates/rustok-fulfillment/storefront/src/transport/graphql_adapter.rs";
const nativeServerFunctionsPath = "crates/rustok-fulfillment/storefront/src/transport/native_server_adapter/server_functions.rs";
const cargoPath = "crates/rustok-fulfillment/storefront/Cargo.toml";
const commerceTransportPath = "crates/rustok-commerce/storefront/src/transport/mod.rs";
const commerceUiPath = "crates/rustok-commerce/storefront/src/ui/leptos/mod.rs";
const planPath = "crates/rustok-fulfillment/docs/implementation-plan.md";
const registryPath = "docs/modules/registry.md";
const packagePath = "package.json";

for (const filePath of [libPath, modelPath, corePath, uiPath, i18nPath, manifestPath, transportPath, graphqlPath, nativeServerFunctionsPath, cargoPath, commerceTransportPath, commerceUiPath, planPath, registryPath, packagePath]) {
  assertExists(filePath, `${filePath}: expected fulfillment storefront FFA file`);
}

const lib = readRepo(libPath);
const model = readRepo(modelPath);
const core = readRepo(corePath);
const ui = readRepo(uiPath);
const i18n = readRepo(i18nPath);
const manifest = readRepo(manifestPath);
const transport = readRepo(transportPath);
const graphql = readRepo(graphqlPath);
const nativeServerFunctions = readRepo(nativeServerFunctionsPath);
const cargo = readRepo(cargoPath);
const commerceTransport = readRepo(commerceTransportPath);
const commerceUi = readRepo(commerceUiPath);
const plan = readRepo(planPath);
const registry = readRepo(registryPath);
const packageJson = readRepo(packagePath);

for (const marker of [
  "mod model;",
  "pub mod core;",
  "FulfillmentShippingSelectionPanel",
  "StorefrontDeliveryGroup",
  "StorefrontShippingOption",
]) {
  assertContains(lib, marker, `${libPath}: expected storefront public surface marker ${marker}`);
}

for (const marker of ["StorefrontDeliveryGroup", "StorefrontShippingOption", "Serialize", "Deserialize"]) {
  assertContains(model, marker, `${modelPath}: expected module-owned DTO marker ${marker}`);
}
assertNotContains(model, "seller_scope", `${modelPath}: fulfillment storefront delivery-group model must not expose legacy seller_scope`);

for (const marker of [
  "SelectShippingOptionRequest",
  "ShippingSelectionLabels",
  "build_select_shipping_option_request",
  "format_shipping_option_price",
]) {
  assertContains(core, marker, `${corePath}: expected core-owned selection helper ${marker}`);
}
for (const marker of ["leptos::", "#[component]", "#[server", "GraphqlRequest", "web_sys::"]) {
  assertNotContains(core, marker, `${corePath}: core must stay UI/transport free (${marker})`);
}

for (const marker of [
  "FulfillmentView",
  "use_context::<UiRouteContext>()",
  "crate::i18n::t",
  "FulfillmentShippingSelectionPanel",
  "build_select_shipping_option_request",
  "on_select_shipping_option",
  "StorefrontDeliveryGroup",
  "StorefrontShippingOption",
]) {
  assertContains(ui, marker, `${uiPath}: expected fulfillment-owned selection UI marker ${marker}`);
}
for (const marker of ["LeptosUiMessages", "include_str!(\"../locales/en.json\")", "include_str!(\"../locales/ru.json\")", "t_for_locale"]) {
  assertContains(i18n, marker, `${i18nPath}: expected host-locale catalog marker ${marker}`);
}
for (const marker of ["slot = \"checkout_shipping_handoff\"", "[provides.storefront_ui.i18n]", "leptos_locales_path = \"storefront/locales\""]) {
  assertContains(manifest, marker, `${manifestPath}: expected locale-aware storefront manifest marker ${marker}`);
}
for (const marker of ["crate::api", "rustok_commerce::", "GraphqlRequest", "#[server"]) {
  assertNotContains(ui, marker, `${uiPath}: UI adapter must not call commerce transport directly (${marker})`);
}

for (const marker of [
  "ShippingSelectionTransportError",
  "select_shipping_option",
  "execute_selected_transport",
  "selected_transport_path",
  "UiTransportError",
  "build_shipping_selection_updates",
  "impl ShippingSelectionError",
  "mod graphql_adapter;",
  "mod native_server_adapter;",
]) {
  assertContains(transport, marker, `${transportPath}: expected owner transport facade marker ${marker}`);
}
for (const marker of [
  "pub seller_scope",
  "seller_scope: Option<String>",
  "normalize_optional(seller_scope)",
]) {
  assertNotContains(transport, marker, `${transportPath}: fulfillment selection transport seam must not expose legacy seller_scope (${marker})`);
}
for (const marker of ["crate::api", "rustok_commerce::", "GraphqlRequest", "#[server"]) {
  assertNotContains(transport, marker, `${transportPath}: owner transport facade must stay host-transport free (${marker})`);
}
for (const marker of ["SELECT_STOREFRONT_SHIPPING_OPTION_MUTATION", "GraphqlRequest::new", "build_shipping_selection_updates"]) {
  assertContains(graphql, marker, `${graphqlPath}: fulfillment must own GraphQL selection marker ${marker}`);
}
assertNotContains(graphql, "rustok_commerce::", `${graphqlPath}: fulfillment GraphQL adapter must not depend on commerce storefront internals`);
assertContains(nativeServerFunctions, "#[server", `${nativeServerFunctionsPath}: fulfillment native server-functions adapter must own a server-function endpoint shell`);
assertContains(nativeServerFunctions, "endpoint = \"fulfillment/select-shipping-option\"", `${nativeServerFunctionsPath}: fulfillment native server-functions adapter must expose the owner endpoint path`);
assertContains(nativeServerFunctions, "rustok_commerce::storefront_checkout_runtime", `${nativeServerFunctionsPath}: fulfillment native server-functions adapter must call the explicit commerce checkout runtime API`);
assertContains(nativeServerFunctions, "expect_context::<HostRuntimeContext>()", `${nativeServerFunctionsPath}: fulfillment native server-functions adapter must use the host runtime context`);
assertContains(nativeServerFunctions, "shared_get::<TransactionalEventBus>()", `${nativeServerFunctionsPath}: fulfillment native server-functions adapter must receive the event bus through the host runtime context`);
assertContains(nativeServerFunctions, "runtime_ctx.db_clone()", `${nativeServerFunctionsPath}: fulfillment native server-functions adapter must receive DB through the host runtime context`);
assertContains(commerceTransport, "select_shipping_option(", `${commerceTransportPath}: commerce SSR adapter must delegate shipping selection fallback policy to fulfillment owner facade`);
assertContains(commerceTransport, "rustok_fulfillment_storefront::transport::select_shipping_option", `${commerceTransportPath}: commerce compatibility adapter must delegate to owner transport facade`);

assertContains(commerceUi, "FulfillmentShippingSelectionPanel", `${commerceUiPath}: commerce host must render fulfillment-owned selection UI`);
assertContains(commerceUi, "transport::select_storefront_shipping_option", `${commerceUiPath}: commerce aggregate UI must call the fulfillment-owned shipping-selection facade`);
assertContains(plan, "verify-fulfillment-storefront-boundary.mjs", `${planPath}: local plan must mention storefront boundary guardrail`);
assertContains(registry, "verify-fulfillment-storefront-boundary.mjs", `${registryPath}: central registry must mention storefront boundary guardrail`);
assertContains(packageJson, "verify:fulfillment:storefront-boundary", `${packagePath}: expected fulfillment storefront boundary script`);
assertContains(packageJson, "npm run verify:fulfillment:storefront-boundary", `${packagePath}: aggregate FFA migration verification must include storefront fulfillment boundary`);

if (failures.length > 0) {
  console.error("fulfillment storefront boundary verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("fulfillment storefront boundary verification passed");

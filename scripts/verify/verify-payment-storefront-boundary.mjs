#!/usr/bin/env node
// RusTok payment storefront FFA boundary guardrails.
// Fast source-level checks for payment-owned checkout action/card UI and request ownership.

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

const libPath = "crates/rustok-payment/storefront/src/lib.rs";
const corePath = "crates/rustok-payment/storefront/src/core.rs";
const transportPath = "crates/rustok-payment/storefront/src/transport.rs";
const graphqlPath = "crates/rustok-payment/storefront/src/transport/graphql_adapter.rs";
const nativeRawPath = "crates/rustok-payment/storefront/src/transport/native_server_adapter/raw_adapter.rs";
const uiPath = "crates/rustok-payment/storefront/src/ui/leptos.rs";
const i18nPath = "crates/rustok-payment/storefront/src/i18n.rs";
const manifestPath = "crates/rustok-payment/rustok-module.toml";
const commerceUiPath = "crates/rustok-commerce/storefront/src/ui/leptos/mod.rs";
const commerceRequestsPath = "crates/rustok-commerce/storefront/src/core/requests.rs";
const planPath = "crates/rustok-payment/docs/implementation-plan.md";
const registryPath = "docs/modules/registry.md";
const packagePath = "package.json";

for (const filePath of [libPath, corePath, transportPath, graphqlPath, nativeRawPath, uiPath, i18nPath, manifestPath, commerceUiPath, commerceRequestsPath, planPath, registryPath, packagePath]) {
  assertExists(filePath, `${filePath}: expected payment storefront FFA file`);
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

for (const marker of ["pub mod core;", "pub mod transport;", "PaymentCollectionActionButton", "PaymentCollectionCard"]) {
  assertContains(lib, marker, `${libPath}: expected storefront public surface marker ${marker}`);
}

for (const marker of [
  "PaymentCollectionCardData",
  "PaymentCollectionCardViewModel",
  "build_payment_collection_card_view_model",
  "PaymentCollectionActionLabels",
  "payment_collection_action_label",
]) {
  assertContains(core, marker, `${corePath}: expected core-owned payment presentation marker ${marker}`);
}
for (const marker of ["leptos::", "#[component]", "#[server", "GraphqlRequest", "web_sys::"]) {
  assertNotContains(core, marker, `${corePath}: core must stay UI/transport free (${marker})`);
}

for (const marker of [
  "PaymentCollectionCreateRequest",
  "PaymentCollection",
  "build_payment_collection_create_request",
  "create_payment_collection",
  "mod graphql_adapter;",
  "mod native_server_adapter;",
  "normalize_required",
]) {
  assertContains(transport, marker, `${transportPath}: expected transport-owned request marker ${marker}`);
}
for (const marker of ["leptos::", "#[component]", "#[server", "GraphqlRequest", "web_sys::"]) {
  assertNotContains(transport, marker, `${transportPath}: transport facade must stay framework/native-endpoint free (${marker})`);
}
for (const marker of ["CREATE_STOREFRONT_PAYMENT_COLLECTION_MUTATION", "GraphqlRequest::new", "PaymentCollection"]) {
  assertContains(graphql, marker, `${graphqlPath}: payment must own GraphQL create/reuse marker ${marker}`);
}
assertNotContains(graphql, "rustok_commerce::", `${graphqlPath}: payment GraphQL adapter must not depend on commerce storefront internals`);
assertContains(nativeRaw, "#[server", `${nativeRawPath}: payment native adapter must own a server-function endpoint shell`);
assertContains(nativeRaw, "endpoint = \"payment/create-payment-collection\"", `${nativeRawPath}: payment native adapter must expose the owner endpoint path`);
assertContains(nativeRaw, "rustok_commerce::storefront_checkout_runtime", `${nativeRawPath}: payment native adapter must call the explicit commerce checkout runtime API`);

for (const marker of [
  "PaymentView",
  "use_context::<UiRouteContext>()",
  "crate::i18n::t",
  "PaymentCollectionActionButton",
  "PaymentCollectionCard",
  "PaymentCollectionCreateRequest",
  "build_payment_collection_create_request",
  "on_create_payment_collection: Callback<PaymentCollectionCreateRequest>",
]) {
  assertContains(ui, marker, `${uiPath}: expected payment-owned UI/request marker ${marker}`);
}
for (const marker of ["include_str!(\"../locales/en.json\")", "include_str!(\"../locales/ru.json\")", "resolve_ui_message_or_fallback"]) {
  assertContains(i18n, marker, `${i18nPath}: expected host-locale catalog marker ${marker}`);
}
for (const marker of ["slot = \"checkout_payment_handoff\"", "[provides.storefront_ui.i18n]", "leptos_locales_path = \"storefront/locales\""]) {
  assertContains(manifest, marker, `${manifestPath}: expected locale-aware storefront manifest marker ${marker}`);
}
for (const marker of ["crate::api", "rustok_commerce::", "GraphqlRequest", "#[server"] ) {
  assertNotContains(ui, marker, `${uiPath}: UI adapter must not call commerce/raw transport directly (${marker})`);
}

assertContains(commerceUi, "PaymentCollectionActionButton", `${commerceUiPath}: commerce host must render payment-owned action UI`);
assertContains(commerceUi, "Callback::new(move |request: PaymentCollectionCreateRequest|", `${commerceUiPath}: commerce callback must accept payment-owned request DTO`);
assertNotContains(commerceUi, "build_payment_collection_command_request", `${commerceUiPath}: commerce UI must not rebuild payment requests from raw cart ids`);
assertContains(commerceRequests, "pub type PaymentCollectionCommandRequest = PaymentCollectionCreateRequest", `${commerceRequestsPath}: commerce transport may keep transitional alias to owner request`);
assertNotContains(commerceRequests, "build_payment_collection_create_request", `${commerceRequestsPath}: commerce core must not wrap payment-owned request construction`);
assertNotContains(commerceRequests, "build_payment_collection_command_request", `${commerceRequestsPath}: commerce core must not expose a payment request builder after owner UI handoff`);
assertContains(plan, "verify-payment-storefront-boundary.mjs", `${planPath}: local plan must mention payment storefront boundary guardrail`);
assertContains(registry, "verify-payment-storefront-boundary.mjs", `${registryPath}: central registry must mention payment storefront boundary guardrail`);
assertContains(packageJson, "verify:payment:storefront-boundary", `${packagePath}: expected payment storefront boundary script`);
assertContains(packageJson, "npm run verify:payment:storefront-boundary", `${packagePath}: aggregate FFA migration verification must include storefront payment boundary`);

if (failures.length > 0) {
  console.error("payment storefront boundary verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("payment storefront boundary verification passed");

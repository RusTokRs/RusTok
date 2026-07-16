#!/usr/bin/env node
// RusTok payment storefront and webhook boundary guardrails.

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

function readJson(relativePath) {
  return JSON.parse(readRepo(relativePath));
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
const nativeServerFunctionsPath = "crates/rustok-payment/storefront/src/transport/native_server_adapter/server_functions.rs";
const cargoPath = "crates/rustok-payment/storefront/Cargo.toml";
const uiPath = "crates/rustok-payment/storefront/src/ui/leptos.rs";
const i18nPath = "crates/rustok-payment/storefront/src/i18n.rs";
const manifestPath = "crates/rustok-payment/rustok-module.toml";
const commerceUiPath = "crates/rustok-commerce/storefront/src/ui/leptos/mod.rs";
const commerceRequestsPath = "crates/rustok-commerce/storefront/src/core/requests.rs";
const planPath = "crates/rustok-commerce/docs/implementation-plan.md";
const paymentPlanRedirectPath = "crates/rustok-payment/docs/implementation-plan.md";
const providerSourcePath = "crates/rustok-payment/src/providers.rs";
const webhookControllerPath = "crates/rustok-payment/src/controllers.rs";
const webhookIngressPath = "crates/rustok-payment/src/services/provider_event_ingress.rs";
const webhookContractPath = "crates/rustok-payment/contracts/payment-provider-webhook-v1.json";
const paymentFbaRegistryPath = "crates/rustok-payment/contracts/payment-fba-registry.json";
const registryPath = "docs/modules/registry.md";
const packagePath = "package.json";

for (const filePath of [
  libPath,
  corePath,
  transportPath,
  graphqlPath,
  nativeServerFunctionsPath,
  cargoPath,
  uiPath,
  i18nPath,
  manifestPath,
  commerceUiPath,
  commerceRequestsPath,
  planPath,
  paymentPlanRedirectPath,
  providerSourcePath,
  webhookControllerPath,
  webhookIngressPath,
  webhookContractPath,
  paymentFbaRegistryPath,
  registryPath,
  packagePath,
]) {
  assertExists(filePath, `${filePath}: expected payment boundary file`);
}

const lib = readRepo(libPath);
const core = readRepo(corePath);
const transport = readRepo(transportPath);
const graphql = readRepo(graphqlPath);
const nativeServerFunctions = readRepo(nativeServerFunctionsPath);
const cargo = readRepo(cargoPath);
const ui = readRepo(uiPath);
const i18n = readRepo(i18nPath);
const manifest = readRepo(manifestPath);
const commerceUi = readRepo(commerceUiPath);
const commerceRequests = readRepo(commerceRequestsPath);
const plan = readRepo(planPath);
const paymentPlanRedirect = readRepo(paymentPlanRedirectPath);
const providerSource = readRepo(providerSourcePath);
const webhookController = readRepo(webhookControllerPath);
const webhookIngress = readRepo(webhookIngressPath);
const webhookContract = readJson(webhookContractPath);
const paymentFbaRegistry = readJson(paymentFbaRegistryPath);
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
  "PaymentCollectionFetchRequest",
  "RefundSummaryFetchRequest",
  "RefundSummary",
  "PaymentCollection",
  "build_payment_collection_fetch_request",
  "build_payment_collection_create_request",
  "fetch_payment_collection",
  "build_refund_summary_fetch_request",
  "fetch_refund_summary",
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
for (const marker of ["STOREFRONT_REFUNDS_QUERY", "STOREFRONT_PAYMENT_COLLECTION_QUERY", "CREATE_STOREFRONT_PAYMENT_COLLECTION_MUTATION", "fetch_refund_summary", "fetch_payment_collection", "GraphqlRequest::new", "RefundSummary", "PaymentCollection"]) {
  assertContains(graphql, marker, `${graphqlPath}: payment must own GraphQL create/reuse marker ${marker}`);
}
assertNotContains(graphql, "rustok_commerce::", `${graphqlPath}: payment GraphQL adapter must not depend on commerce storefront internals`);
assertContains(nativeServerFunctions, "#[server", `${nativeServerFunctionsPath}: payment native server-functions adapter must own a server-function endpoint shell`);
assertContains(nativeServerFunctions, "endpoint = \"payment/create-payment-collection\"", `${nativeServerFunctionsPath}: payment native server-functions adapter must expose the owner endpoint path`);
assertContains(nativeServerFunctions, "endpoint = \"payment/payment-collection\"", `${nativeServerFunctionsPath}: payment native server-functions adapter must expose the owner read endpoint path`);
assertContains(nativeServerFunctions, "endpoint = \"payment/refund-summary\"", `${nativeServerFunctionsPath}: payment native server-functions adapter must expose the owner refund-summary endpoint path`);
assertContains(nativeServerFunctions, "read_storefront_payment_collection", `${nativeServerFunctionsPath}: payment native read adapter must call the access-checked commerce runtime API`);
assertContains(nativeServerFunctions, "read_storefront_order_refunds", `${nativeServerFunctionsPath}: payment refund adapter must call the access-checked commerce runtime API`);
assertContains(nativeServerFunctions, "rustok_commerce::storefront_checkout_runtime", `${nativeServerFunctionsPath}: payment native server-functions adapter must call the explicit commerce checkout runtime API`);
assertContains(nativeServerFunctions, "expect_context::<HostRuntimeContext>()", `${nativeServerFunctionsPath}: payment native server-functions adapter must use the host runtime context`);
assertContains(nativeServerFunctions, "shared_get::<TransactionalEventBus>()", `${nativeServerFunctionsPath}: payment native server-functions adapter must receive the event bus through the host runtime context`);
assertContains(nativeServerFunctions, "runtime_ctx.db_clone()", `${nativeServerFunctionsPath}: payment native server-functions adapter must receive DB through the host runtime context`);

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
for (const marker of ["LeptosUiMessages", "include_str!(\"../locales/en.json\")", "include_str!(\"../locales/ru.json\")", "t_for_locale"]) {
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
assertContains(commerceRequests, "pub type PaymentCollectionCommandRequest = PaymentCollectionCreateRequest", `${commerceRequestsPath}: commerce transport must keep the owner request alias for aggregate checkout composition`);
assertNotContains(commerceRequests, "build_payment_collection_create_request", `${commerceRequestsPath}: commerce core must not wrap payment-owned request construction`);
assertNotContains(commerceRequests, "build_payment_collection_command_request", `${commerceRequestsPath}: commerce core must not expose a payment request builder after owner UI handoff`);

for (const marker of [
  "pub delivery_id: Option<String>",
  "pub idempotency_key: Option<String>",
  "pub delivery_id: String",
  "Transport identity values are untrusted hints",
  "validate_verified_webhook_result",
  "conflicts with the transport hint",
]) {
  assertContains(providerSource, marker, `${providerSourcePath}: missing verified webhook identity marker ${marker}`);
}
assertContains(webhookController, "optional_normalized_header", `${webhookControllerPath}: identity headers must be optional hints`);
assertNotContains(webhookController, "fn required_header(", `${webhookControllerPath}: custom delivery/replay headers must not be required`);
assertContains(webhookIngress, "delivery_id: normalized.delivery_id.clone()", `${webhookIngressPath}: inbox delivery identity must come from verified provider result`);
assertContains(webhookIngress, "idempotency_key: normalized.replay_key.clone()", `${webhookIngressPath}: inbox replay identity must come from verified provider result`);
if (webhookContract.transport?.identity_hint_policy !== "untrusted-cross-check-only") {
  fail(`${webhookContractPath}: identity hints must be untrusted cross-checks only`);
}
if (webhookContract.inbox?.identity_source !== "signature-verified-provider-result") {
  fail(`${webhookContractPath}: durable identity must come from the verified provider result`);
}
if (webhookContract.security?.transport_identity_headers_authoritative !== false) {
  fail(`${webhookContractPath}: transport identity headers must not be authoritative`);
}
const webhookIngressContract = paymentFbaRegistry.provider_spi?.webhook_ingress;
if (
  webhookIngressContract?.verified_identity_required !== true ||
  webhookIngressContract?.transport_identity_headers_required !== false ||
  webhookIngressContract?.transport_identity_headers_authoritative !== false
) {
  fail(`${paymentFbaRegistryPath}: verified webhook identity policy drift`);
}

assertContains(plan, "## Payment workstream", `${planPath}: main ecommerce plan must own the payment workstream`);
assertContains(plan, "verify-payment-storefront-boundary.mjs", `${planPath}: main ecommerce plan must mention payment storefront boundary guardrail`);
assertContains(plan, "signature-verified provider result", `${planPath}: main ecommerce plan must record authoritative webhook identity`);
assertContains(paymentPlanRedirect, "crates/rustok-commerce/docs/implementation-plan.md#payment-workstream", `${paymentPlanRedirectPath}: payment planning must redirect to the main ecommerce workstream`);
for (const forbiddenMarker of ["- [x]", "- [ ]", "## Immediate execution order", "## Verification and promotion checklist"]) {
  assertNotContains(paymentPlanRedirect, forbiddenMarker, `${paymentPlanRedirectPath}: payment redirect must not maintain a second roadmap (${forbiddenMarker})`);
}
assertContains(registry, "verify-payment-storefront-boundary.mjs", `${registryPath}: central registry must mention payment storefront boundary guardrail`);
assertContains(packageJson, "verify:payment:storefront-boundary", `${packagePath}: expected payment storefront boundary script`);
assertContains(packageJson, "npm run verify:payment:storefront-boundary", `${packagePath}: aggregate FFA migration verification must include storefront payment boundary`);

if (failures.length > 0) {
  console.error("payment boundary verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("payment boundary verification passed");

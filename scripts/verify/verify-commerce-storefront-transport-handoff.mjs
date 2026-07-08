#!/usr/bin/env node
// RusTok commerce storefront transport handoff guardrails.
// Fast source-level checks that aggregate checkout keeps owner DTOs and selects native
// or GraphQL through the shared UI transport policy.

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

function assertStructNotContains(text, structName, pattern, description) {
  const start = text.indexOf(`struct ${structName}`);
  if (start === -1) {
    fail(`${description} (struct ${structName} not found)`);
    return;
  }
  const bodyStart = text.indexOf("{", start);
  const bodyEnd = text.indexOf("\n}", bodyStart);
  if (bodyStart === -1 || bodyEnd === -1) {
    fail(`${description} (struct ${structName} body not found)`);
    return;
  }
  assertNotContains(text.slice(bodyStart, bodyEnd), pattern, description);
}

const requestsPath = "crates/rustok-commerce/storefront/src/core/requests.rs";
const presentationPath = "crates/rustok-commerce/storefront/src/core/presentation.rs";
const modelPath = "crates/rustok-commerce/storefront/src/model.rs";
const libPath = "crates/rustok-commerce/storefront/src/lib.rs";
const uiPath = "crates/rustok-commerce/storefront/src/ui/leptos/mod.rs";
const transportPath = "crates/rustok-commerce/storefront/src/transport/mod.rs";
const nativePath = "crates/rustok-commerce/storefront/src/transport/native_server_adapter.rs";
const graphqlPath = "crates/rustok-commerce/storefront/src/transport/graphql_adapter.rs";
const runtimePath = "crates/rustok-commerce/src/storefront_checkout_runtime.rs";
const storeControllerPath = "crates/rustok-commerce/src/controllers/store/mod.rs";
const graphqlTypesPath = "crates/rustok-commerce/src/graphql/types.rs";
const legacyApiPath = "crates/rustok-commerce/storefront/src/api.rs";
const paymentTransportPath = "crates/rustok-payment/storefront/src/transport.rs";
const paymentGraphqlPath = "crates/rustok-payment/storefront/src/transport/graphql_adapter.rs";
const paymentNativeServerFunctionsPath = "crates/rustok-payment/storefront/src/transport/native_server_adapter/server_functions.rs";
const orderTransportPath = "crates/rustok-order/storefront/src/transport.rs";
const orderGraphqlPath = "crates/rustok-order/storefront/src/transport/graphql_adapter.rs";
const orderNativeServerFunctionsPath = "crates/rustok-order/storefront/src/transport/native_server_adapter/server_functions.rs";
const fulfillmentTransportPath = "crates/rustok-fulfillment/storefront/src/transport.rs";
const fulfillmentGraphqlPath = "crates/rustok-fulfillment/storefront/src/transport/graphql_adapter.rs";
const fulfillmentNativeServerFunctionsPath = "crates/rustok-fulfillment/storefront/src/transport/native_server_adapter/server_functions.rs";
const commercePlanPath = "crates/rustok-commerce/docs/implementation-plan.md";
const paymentPlanPath = "crates/rustok-payment/docs/implementation-plan.md";
const orderPlanPath = "crates/rustok-order/docs/implementation-plan.md";
const fulfillmentPlanPath = "crates/rustok-fulfillment/docs/implementation-plan.md";
const registryPath = "docs/modules/registry.md";
const packagePath = "package.json";

for (const filePath of [requestsPath, presentationPath, modelPath, libPath, uiPath, transportPath, nativePath, graphqlPath, runtimePath, storeControllerPath, graphqlTypesPath, paymentTransportPath, paymentGraphqlPath, paymentNativeServerFunctionsPath, orderTransportPath, orderGraphqlPath, orderNativeServerFunctionsPath, fulfillmentTransportPath, fulfillmentGraphqlPath, fulfillmentNativeServerFunctionsPath, commercePlanPath, paymentPlanPath, orderPlanPath, fulfillmentPlanPath, registryPath, packagePath]) {
  assertExists(filePath, `${filePath}: expected storefront transport handoff file`);
}
if (existsSync(repoPath(legacyApiPath))) {
  fail(`${legacyApiPath}: commerce storefront legacy api.rs must stay removed; transport/native_server_adapter.rs owns native operations`);
}

const requests = readRepo(requestsPath);
const presentation = readRepo(presentationPath);
const model = readRepo(modelPath);
const lib = readRepo(libPath);
const ui = readRepo(uiPath);
const transport = readRepo(transportPath);
const nativeAdapter = readRepo(nativePath);
const graphqlAdapter = readRepo(graphqlPath);
const runtimeApi = readRepo(runtimePath);
const storeController = readRepo(storeControllerPath);
const graphqlTypes = readRepo(graphqlTypesPath);
const paymentTransport = readRepo(paymentTransportPath);
const paymentGraphql = readRepo(paymentGraphqlPath);
const paymentNativeServerFunctions = readRepo(paymentNativeServerFunctionsPath);
const orderTransport = readRepo(orderTransportPath);
const orderGraphql = readRepo(orderGraphqlPath);
const orderNativeServerFunctions = readRepo(orderNativeServerFunctionsPath);
const fulfillmentTransport = readRepo(fulfillmentTransportPath);
const fulfillmentGraphql = readRepo(fulfillmentGraphqlPath);
const fulfillmentNativeServerFunctions = readRepo(fulfillmentNativeServerFunctionsPath);
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
assertNotContains(requests, "seller_scope: None", `${requestsPath}: aggregate checkout must not pass legacy seller_scope into fulfillment owner transport DTOs`);
assertNotContains(model, "seller_scope", `${modelPath}: commerce storefront checkout delivery-group model must not expose legacy seller_scope`);
for (const marker of [
  "build_payment_collection_command_request",
  "build_checkout_completion_command_request",
  "pub struct PaymentCollectionCommandRequest",
  "pub struct CheckoutCompletionCommandRequest",
]) {
  assertNotContains(requests, marker, `${requestsPath}: aggregate checkout must not recreate owner command DTOs (${marker})`);
}
for (const marker of [
  "build_fulfillment_delivery_groups",
  "build_fulfillment_shipping_selection_labels",
  "build_cart_checkout_handoff_labels",
  "build_payment_collection_action_labels",
  "build_payment_collection_card_data",
  "build_payment_collection_card_labels",
  "build_order_checkout_action_labels",
  "build_order_checkout_result_data",
  "build_order_checkout_result_labels",
]) {
  assertContains(presentation, marker, `${presentationPath}: Leptos-free core must own checkout owner-fragment mapper ${marker}`);
}
for (const marker of [
  "fn fulfillment_delivery_groups",
  "fn fulfillment_shipping_selection_labels",
  "fn cart_checkout_handoff_labels",
  "fn payment_collection_action_labels",
  "fn payment_collection_card_data",
  "fn payment_collection_card_labels",
  "fn order_checkout_action_labels",
  "fn order_checkout_result_data",
  "fn order_checkout_result_labels",
]) {
  assertNotContains(ui, marker, `${uiPath}: checkout owner-fragment presentation helpers must stay in core (${marker})`);
}
for (const marker of ["leptos::", "#[component]", "#[server", "GraphqlRequest", "web_sys::"]) {
  assertNotContains(presentation, marker, `${presentationPath}: commerce storefront presentation core must stay UI/transport free (${marker})`);
}

for (const operation of [
  "fetch_storefront_commerce",
  "create_storefront_payment_collection",
  "select_storefront_shipping_option",
  "complete_storefront_checkout",
]) {
  assertContains(transport, `pub async fn ${operation}`, `${transportPath}: missing transport operation ${operation}`);
}
assertContains(transport, "execute_selected_transport", `${transportPath}: native/GraphQL selection must use shared UI transport policy`);
for (const marker of [
  "create_payment_collection",
  "complete_checkout",
  "select_shipping_option",
]) {
  assertContains(transport, marker, `${transportPath}: aggregate checkout must delegate owner handoff through ${marker}`);
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
assertContains(transport, "mod native_server_adapter;", `${transportPath}: transport facade must wire native server adapter inside transport boundary`);
assertContains(transport, "use shared_adapter::ApiError;", `${transportPath}: transport facade must expose ApiError from shared adapter`);
assertNotContains(transport, "crate::api", `${transportPath}: transport facade must not delegate to legacy api module`);
assertNotContains(nativeAdapter, "crate::api", `${nativePath}: native adapter must not delegate to legacy api module`);
assertNotContains(graphqlAdapter, "crate::api", `${graphqlPath}: GraphQL adapter must not delegate to legacy api module`);
assertContains(nativeAdapter, "#[server", `${nativePath}: native adapter must keep native server-function endpoints`);
assertNotContains(nativeAdapter, "GraphqlRequest", `${nativePath}: commerce aggregate adapter must delegate owner GraphQL reads instead of issuing raw GraphQL requests`);
assertContains(nativeAdapter, "endpoint = \"commerce/storefront-data\"", `${nativePath}: commerce native adapter must keep only the aggregate storefront data server-function endpoint`);
assertContains(nativeAdapter, "rustok_cart_storefront::transport::fetch_cart", `${nativePath}: commerce aggregate read must delegate cart workspace loading to the cart owner transport`);
assertContains(nativeAdapter, "rustok_cart_storefront::core::build_cart_fetch_request", `${nativePath}: commerce aggregate read must build cart owner requests through cart core`);
assertContains(nativeAdapter, "rustok_payment_storefront::transport::fetch_payment_collection", `${nativePath}: commerce aggregate read must delegate payment collection loading to the payment owner transport`);
assertContains(nativeAdapter, "rustok_payment_storefront::transport::build_payment_collection_fetch_request", `${nativePath}: commerce aggregate read must build payment owner requests through its transport contract`);
assertContains(nativeAdapter, "map_cart_checkout_cart", `${nativePath}: commerce aggregate read may only convert from cart-owned DTOs into its checkout aggregate model`);
for (const marker of [
  "STOREFRONT_CHECKOUT_QUERY",
  "StorefrontCheckoutResponse",
  "GraphqlCheckoutCart",
  "GraphqlCheckoutDeliveryGroup",
  "GraphqlCheckoutShippingOption",
  "map_graphql_checkout_cart",
  "map_native_checkout_cart",
  "rustok_cart::CartService",
  "rustok_payment::PaymentService",
  "map_native_payment_collection",
  "STOREFRONT_REFUNDS_QUERY",
  "StorefrontOrderRefundSummary",
  "summarize_storefront_refunds",
  "fetch_storefront_order_refunds_summary",
  "reprice_storefront_cart_line_items",
]) {
  assertNotContains(nativeAdapter, marker, `${nativePath}: commerce native adapter must not own cart read implementation (${marker})`);
}
for (const endpoint of ["commerce/create-payment-collection", "commerce/select-shipping-option", "commerce/complete-checkout"]) {
  assertNotContains(nativeAdapter, endpoint, `${nativePath}: commerce native adapter must not own owner operation endpoint ${endpoint}`);
}
assertContains(runtimeApi, "pub async fn create_storefront_payment_collection", `${runtimePath}: commerce runtime API must expose payment collection orchestration`);
assertContains(runtimeApi, "pub async fn read_storefront_order_refunds", `${runtimePath}: commerce runtime API must expose access-checked order refund reads to the payment owner adapter`);
assertContains(runtimeApi, "pub async fn select_storefront_shipping_option", `${runtimePath}: commerce runtime API must expose shipping selection orchestration`);
assertContains(runtimeApi, "pub async fn complete_storefront_checkout", `${runtimePath}: commerce runtime API must expose checkout completion orchestration`);
assertContains(runtimeApi, "StorefrontPaymentCollectionCommand", `${runtimePath}: runtime API must use typed payment command input`);
assertContains(runtimeApi, "StorefrontShippingSelectionCommand", `${runtimePath}: runtime API must use typed fulfillment command input`);
assertContains(runtimeApi, "StorefrontCheckoutCompletionCommand", `${runtimePath}: runtime API must use typed order command input`);
assertNotContains(nativeAdapter, "build_shipping_selection_updates", `${nativePath}: commerce native adapter must not consume fulfillment-owned shipping selection materialization`);
assertNotContains(nativeAdapter, "build_shipping_selection_plan", `${nativePath}: commerce native adapter must not own shipping selection planning`);
assertNotContains(nativeAdapter, "fn shipping_selection_error_message", `${nativePath}: commerce native adapter must not own fulfillment selection error text`);
assertNotContains(nativeAdapter, "sellerScope lineItemIds", `${nativePath}: checkout read query must not request legacy sellerScope for delivery-group matching`);
assertNotContains(nativeAdapter, "serde(rename = \"sellerScope\")", `${nativePath}: storefront GraphQL fallback selection payload must not send legacy sellerScope`);
assertStructNotContains(storeController, "StoreCartShippingSelectionInput", "seller_scope", `${storeControllerPath}: REST storefront shipping selection input must not accept legacy seller_scope`);
assertStructNotContains(graphqlTypes, "StorefrontShippingSelectionInput", "seller_scope", `${graphqlTypesPath}: GraphQL storefront shipping selection input must not accept legacy seller_scope`);
assertStructNotContains(graphqlTypes, "GqlCartLineItem", "seller_scope", `${graphqlTypesPath}: GraphQL cart line item output must not expose legacy seller_scope`);
assertStructNotContains(graphqlTypes, "GqlCartDeliveryGroup", "seller_scope", `${graphqlTypesPath}: GraphQL cart delivery group output must not expose legacy seller_scope`);
for (const marker of [
  "CREATE_STOREFRONT_PAYMENT_COLLECTION_MUTATION",
  "COMPLETE_STOREFRONT_CHECKOUT_MUTATION",
  "SELECT_STOREFRONT_SHIPPING_OPTION_MUTATION",
  "create_storefront_payment_collection_graphql",
  "complete_storefront_checkout_graphql",
  "select_storefront_shipping_option_graphql",
  "GraphqlPaymentCollection",
  "GraphqlCheckoutCompletion",
]) {
  assertNotContains(nativeAdapter, marker, `${nativePath}: owner checkout GraphQL implementation must not remain in commerce (${marker})`);
  assertNotContains(graphqlAdapter, marker, `${graphqlPath}: commerce GraphQL wrapper must not reintroduce owner operation ${marker}`);
}

for (const [ownerTransport, ownerPath, operation, errorType] of [
  [paymentTransport, paymentTransportPath, "create_payment_collection", "PaymentTransportError"],
  [orderTransport, orderTransportPath, "complete_checkout", "CheckoutCompletionTransportError"],
  [fulfillmentTransport, fulfillmentTransportPath, "select_shipping_option", "ShippingSelectionTransportError"],
]) {
  assertContains(ownerTransport, `pub enum ${errorType}`, `${ownerPath}: owner transport must expose typed fallback error ${errorType}`);
  assertContains(ownerTransport, `pub async fn ${operation}`, `${ownerPath}: owner transport must expose owner GraphQL fallback facade ${operation}`);
  assertContains(ownerTransport, "mod native_server_adapter;", `${ownerPath}: owner transport facade must wire its native server adapter`);
  assertContains(ownerTransport, "execute_selected_transport", `${ownerPath}: owner fallback facade must use shared UI transport selection`);
  assertContains(ownerTransport, "mod graphql_adapter;", `${ownerPath}: owner transport facade must wire its GraphQL adapter`);
}
assertContains(fulfillmentTransport, "build_shipping_selection_updates", `${fulfillmentTransportPath}: fulfillment transport must own shipping selection materialization for compatibility cutover`);

assertNotContains(nativeAdapter, "create_storefront_payment_collection", `${nativePath}: commerce native adapter must not keep payment owner operation wrapper`);
assertNotContains(nativeAdapter, "complete_storefront_checkout", `${nativePath}: commerce native adapter must not keep order owner operation wrapper`);
assertNotContains(nativeAdapter, "select_storefront_shipping_option", `${nativePath}: commerce native adapter must not keep fulfillment owner operation wrapper`);
assertContains(paymentNativeServerFunctions, "endpoint = \"payment/create-payment-collection\"", `${paymentNativeServerFunctionsPath}: payment must own native payment collection endpoint shell`);
assertContains(paymentNativeServerFunctions, "rustok_commerce::storefront_checkout_runtime", `${paymentNativeServerFunctionsPath}: payment native endpoint must call explicit commerce checkout runtime API`);
assertContains(orderNativeServerFunctions, "endpoint = \"order/complete-checkout\"", `${orderNativeServerFunctionsPath}: order must own native checkout completion endpoint shell`);
assertContains(orderNativeServerFunctions, "rustok_commerce::storefront_checkout_runtime", `${orderNativeServerFunctionsPath}: order native endpoint must call explicit commerce checkout runtime API`);
assertContains(fulfillmentNativeServerFunctions, "endpoint = \"fulfillment/select-shipping-option\"", `${fulfillmentNativeServerFunctionsPath}: fulfillment must own native shipping selection endpoint shell`);
assertContains(fulfillmentNativeServerFunctions, "rustok_commerce::storefront_checkout_runtime", `${fulfillmentNativeServerFunctionsPath}: fulfillment native endpoint must call explicit commerce checkout runtime API`);
for (const [ownerGraphql, ownerPath, mutation, operation] of [
  [paymentGraphql, paymentGraphqlPath, "CREATE_STOREFRONT_PAYMENT_COLLECTION_MUTATION", "create_payment_collection"],
  [orderGraphql, orderGraphqlPath, "COMPLETE_STOREFRONT_CHECKOUT_MUTATION", "complete_checkout"],
  [fulfillmentGraphql, fulfillmentGraphqlPath, "SELECT_STOREFRONT_SHIPPING_OPTION_MUTATION", "select_shipping_option"],
]) {
  assertContains(ownerGraphql, mutation, `${ownerPath}: owner GraphQL adapter must own mutation ${mutation}`);
  assertContains(ownerGraphql, `pub(super) async fn ${operation}`, `${ownerPath}: owner GraphQL adapter must expose internal operation ${operation}`);
  assertContains(ownerGraphql, "GraphqlRequest::new", `${ownerPath}: owner GraphQL adapter must execute the public GraphQL contract`);
}

assertContains(commercePlan, "verify-commerce-storefront-transport-handoff.mjs", `${commercePlanPath}: commerce plan must mention transport handoff guardrail`);
assertContains(commercePlan, "storefront/src/transport/native_server_adapter.rs", `${commercePlanPath}: commerce plan must document storefront native adapter location`);
for (const operation of ["create_payment_collection", "fetch_payment_collection", "fetch_refund_summary"]) {
  assertContains(paymentPlan, `\`${operation}\``, `${paymentPlanPath}: payment plan must document owner GraphQL fallback policy for ${operation}`);
}
assertContains(paymentPlan, "execute_selected_transport", `${paymentPlanPath}: payment plan must document shared transport selection policy`);
assertContains(orderPlan, "execute_selected_transport", `${orderPlanPath}: order plan must document shared transport selection policy`);
assertContains(fulfillmentPlan, "execute_selected_transport", `${fulfillmentPlanPath}: fulfillment plan must document shared transport selection policy`);
assertContains(registry, "verify-commerce-storefront-transport-handoff.mjs", `${registryPath}: central registry must mention transport handoff guardrail`);
assertContains(registry, "storefront/src/transport/native_server_adapter.rs", `${registryPath}: central registry must document commerce storefront native adapter location`);
assertContains(packageJson, "verify:commerce:storefront-transport-handoff", `${packagePath}: expected transport handoff script`);
assertContains(packageJson, "npm run verify:commerce:storefront-transport-handoff", `${packagePath}: aggregate FFA migration verification must include transport handoff guardrail`);

if (failures.length > 0) {
  console.error("commerce storefront transport handoff verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("commerce storefront transport handoff verification passed");

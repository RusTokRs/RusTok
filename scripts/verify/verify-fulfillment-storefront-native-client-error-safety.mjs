#!/usr/bin/env node

import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? path.resolve(configuredRoot)
  : path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const read = (relativePath) => readFileSync(path.join(root, relativePath), "utf8");
const failures = [];

const requireText = (source, value, label) => {
  if (!source.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (source, value, label) => {
  if (source.includes(value)) failures.push(`${label}: forbidden ${value}`);
};
const requireCount = (source, value, expected, label) => {
  const count = source.split(value).length - 1;
  if (count !== expected) failures.push(`${label}: expected ${expected}, found ${count}`);
};

const paths = {
  transport: "crates/rustok-fulfillment/storefront/src/transport.rs",
  adapter: "crates/rustok-fulfillment/storefront/src/transport/native_server_adapter.rs",
  safety:
    "crates/rustok-fulfillment/storefront/src/transport/native_server_adapter/native_client_error_safety.rs",
  serverFunctions:
    "crates/rustok-fulfillment/storefront/src/transport/native_server_adapter/server_functions.rs",
  graphqlAdapter:
    "crates/rustok-fulfillment/storefront/src/transport/graphql_adapter.rs",
  graphqlSafety:
    "crates/rustok-fulfillment/storefront/src/transport/graphql_error_safety.rs",
  evidence:
    "crates/rustok-fulfillment/contracts/evidence/storefront-native-client-error-safety-source.json",
  review:
    "crates/rustok-fulfillment/contracts/evidence/storefront-native-client-error-safety-source-review.json",
  doc: "crates/rustok-fulfillment/docs/storefront-native-error-safety.md",
  commercePlan: "crates/rustok-commerce/docs/implementation-plan.md",
  nativeGuard: "scripts/verify/verify-fulfillment-storefront-native-error-safety.mjs",
  graphqlGuard: "scripts/verify/verify-fulfillment-storefront-graphql-error-safety.mjs",
};

const transport = read(paths.transport);
const adapter = read(paths.adapter);
const safety = read(paths.safety);
const serverFunctions = read(paths.serverFunctions);
const graphqlAdapter = read(paths.graphqlAdapter);
const graphqlSafety = read(paths.graphqlSafety);
const evidence = JSON.parse(read(paths.evidence));
const review = JSON.parse(read(paths.review));
const doc = read(paths.doc);
const commercePlan = read(paths.commercePlan);
const nativeGuard = read(paths.nativeGuard);
const graphqlGuard = read(paths.graphqlGuard);

for (const [value, label] of [
  ["mod graphql_adapter;", "GraphQL module"],
  ["mod graphql_error_safety;", "GraphQL safety module"],
  ["mod native_server_adapter;", "native adapter module"],
  ["execute_selected_transport(", "explicit transport selection"],
  [
    "move || native_server_adapter::select_shipping_option(native_request)",
    "unchanged native facade closure",
  ],
  [
    "let context = graphql_error_safety::GraphqlCallContext::new(&request);",
    "unchanged GraphQL context",
  ],
  ["graphql_adapter::select_shipping_option(request)", "unchanged GraphQL call"],
  [".map_err(|error| context.map_error(error))", "unchanged GraphQL mapping"],
  ["UiTransportPath::NativeServer", "native path"],
  ["UiTransportPath::Graphql", "GraphQL path"],
]) requireText(transport, value, `${paths.transport}: ${label}`);
for (const value of [
  "fallback_failed(",
  "native_client_error_safety",
  "NativeClientErrorContext",
]) forbidText(transport, value, `${paths.transport}: facade must stay unchanged`);

for (const [value, label] of [
  ["mod native_client_error_safety;", "private safety module"],
  ["mod server_functions;", "server-functions module"],
  ["use self::native_client_error_safety::NativeClientErrorContext;", "context import"],
  ["let context = NativeClientErrorContext::validate_and_new(&request)?;", "preflight context"],
  ["server_functions::select_shipping_option_server(request)", "unchanged server call"],
  [".map_err(|error| context.map_error(error))", "final native mapping"],
]) requireText(adapter, value, `${paths.adapter}: ${label}`);

const contextIndex = adapter.indexOf("NativeClientErrorContext::validate_and_new(&request)");
const callIndex = adapter.indexOf("server_functions::select_shipping_option_server(request)");
const mapIndex = adapter.indexOf("context.map_error(error)");
if (!(contextIndex >= 0 && callIndex > contextIndex && mapIndex > callIndex)) {
  failures.push(`${paths.adapter}: expected validation/context -> server call -> final mapping order`);
}

for (const [value, label] of [
  [
    'const FULFILLMENT_STOREFRONT_NATIVE_CLIENT_OWNER: &str = "rustok_fulfillment.storefront";',
    "owner constant",
  ],
  [
    '"select_storefront_shipping_option"',
    "owner operation",
  ],
  [
    '"fulfillment_storefront_native_client_transport"',
    "client boundary",
  ],
  [
    '"Shipping selection request could not be completed"',
    "static technical public message",
  ],
  ["pub(super) struct NativeClientErrorContext", "private context"],
  ["pub(super) fn validate_and_new(", "validation constructor"],
  ["Uuid::parse_str(request.cart_id.trim())", "cart UUID preflight"],
  ["let updates = build_shipping_selection_updates(request)?;", "selection-plan preflight"],
  ["Uuid::parse_str(option_id.trim())", "selected-option UUID preflight"],
  ['"cart_id must be a valid UUID"', "cart validation message"],
  [
    '"selected_shipping_option_id must be a valid UUID"',
    "option validation message",
  ],
  ["ShippingSelectionTransportError::Validation(message) =>", "validation pass-through"],
  ["raw_error = ?error", "private raw cause"],
  ["correlation_id = %self.correlation_id", "correlation diagnostics"],
  ["cart_id_length = self.cart_id_length", "cart length fact"],
  ["delivery_group_count = self.delivery_group_count", "group count fact"],
  [
    "shipping_profile_slug_length = self.shipping_profile_slug_length",
    "profile length fact",
  ],
  ["seller_id_present = self.seller_id_present", "seller presence fact"],
  [
    "shipping_option_id_present = self.shipping_option_id_present",
    "option presence fact",
  ],
  [
    "available_shipping_option_count = self.available_shipping_option_count",
    "available-option count fact",
  ],
  [
    'code = "fulfillment.storefront_native_client_transport_failed"',
    "stable client code",
  ],
  ["boundary = FULFILLMENT_STOREFRONT_NATIVE_CLIENT_BOUNDARY", "boundary diagnostics"],
  ["ShippingSelectionTransportError::ServerFn(", "static technical remap"],
]) requireText(safety, value, `${paths.safety}: ${label}`);

const cartValidationIndex = safety.indexOf("Uuid::parse_str(request.cart_id.trim())");
const selectionValidationIndex = safety.indexOf("build_shipping_selection_updates(request)?");
const optionValidationIndex = safety.indexOf("Uuid::parse_str(option_id.trim())");
if (!(cartValidationIndex >= 0 && selectionValidationIndex > cartValidationIndex && optionValidationIndex > selectionValidationIndex)) {
  failures.push(`${paths.safety}: native validation order must remain cart -> selection plan -> option`);
}

for (const value of [
  "cart_id = %",
  "cart_id = ?",
  "shipping_profile_slug = %",
  "shipping_profile_slug = ?",
  "seller_id = %",
  "seller_id = ?",
  "shipping_option_id = %",
  "shipping_option_id = ?",
  "delivery_groups =",
  "available_shipping_option_ids =",
  "request = ?",
  "error.to_string()",
]) forbidText(safety, value, `${paths.safety}: raw request or error text`);

for (const [value, label] of [
  [
    'endpoint = "fulfillment/select-shipping-option"',
    "mounted endpoint",
  ],
  ["build_shipping_selection_updates(&request)", "server-side selection validation"],
  [
    'ServerFnError::new("cart_id must be a valid UUID")',
    "server-side cart validation",
  ],
  [
    'ServerFnError::new(format!("{field_name} must be a valid UUID"))',
    "server-side option validation",
  ],
  ["select_storefront_shipping_option(", "owner runtime call"],
  [
    "ShippingSelectionTransportError::ServerFn(error.to_string())",
    "compatibility handoff",
  ],
]) requireText(serverFunctions, value, `${paths.serverFunctions}: ${label}`);
requireCount(
  serverFunctions,
  "ShippingSelectionTransportError::ServerFn(error.to_string())",
  1,
  `${paths.serverFunctions}: one compatibility handoff`,
);

requireText(
  graphqlAdapter,
  "ShippingSelectionTransportError::Graphql(error.to_string())",
  `${paths.graphqlAdapter}: GraphQL handoff`,
);
requireText(
  graphqlSafety,
  "pub(super) struct GraphqlCallContext",
  `${paths.graphqlSafety}: GraphQL policy`,
);
requireText(
  nativeGuard,
  "FULFILLMENT_STOREFRONT_NATIVE_OWNER",
  `${paths.nativeGuard}: mounted native guard`,
);
requireText(
  graphqlGuard,
  "move || native_server_adapter::select_shipping_option(native_request)",
  `${paths.graphqlGuard}: unchanged facade assertion`,
);

if (evidence.status !== "fulfillment_storefront_native_client_error_safety_source_unvalidated") {
  failures.push(`${paths.evidence}: unexpected status ${evidence.status}`);
}
if (
  review.status !==
  "fulfillment_storefront_native_client_error_safety_source_reviewed_unvalidated"
) {
  failures.push(`${paths.review}: unexpected status ${review.status}`);
}

for (const [key, expected] of Object.entries({
  operation_count: 1,
  transport_facade_changed: false,
  graphql_transport_changed: false,
  server_functions_changed: false,
  request_response_dto_changed: false,
  shipping_selection_policy_changed: false,
  native_adapter_final_mapping: true,
  validation_preflight_preserved: true,
  validation_order_preserved: true,
  cart_uuid_validation_message_preserved: true,
  selection_plan_validation_messages_preserved: true,
  shipping_option_uuid_validation_message_preserved: true,
  technical_native_error_public: false,
  static_technical_public_message: true,
  context_created_before_server_function_call: true,
  original_error_logged_privately: true,
  per_call_correlation_logging: true,
  safe_request_shape_only: true,
  cart_id_values_logged: false,
  shipping_profile_slug_values_logged: false,
  seller_id_values_logged: false,
  shipping_option_id_values_logged: false,
  delivery_group_values_logged: false,
  fallback_enabled: false,
  broad_ecommerce_cleanup_closed: false,
})) {
  if (evidence.source_contract?.[key] !== expected) {
    failures.push(`${paths.evidence}: source_contract.${key} must be ${expected}`);
  }
}

for (const key of [
  "tests_run",
  "cargo_run",
  "format_run",
  "verifiers_run",
  "workflow_checks_run",
  "ci_run",
  "hydrate_compile_proven",
  "ssr_compile_proven",
  "browser_runtime_proven",
  "mounted_runtime_proven",
]) {
  if (evidence.validation?.[key] !== false) {
    failures.push(`${paths.evidence}: validation.${key} must remain false`);
  }
}

requireText(doc, "Status: **source-ready / unvalidated**", `${paths.doc}: status`);
requireText(
  doc,
  "Shipping selection request could not be completed",
  `${paths.doc}: static public message`,
);
requireText(
  commercePlan,
  "Finish correlation-safe mapper cleanup",
  `${paths.commercePlan}: broad ecommerce cleanup remains open`,
);

if (failures.length > 0) {
  console.error("Fulfillment storefront native client error-safety verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  "Fulfillment storefront native client technical failures use a correlation-safe static envelope while validation, GraphQL, and mounted server-function contracts remain unchanged; execution evidence remains open",
);

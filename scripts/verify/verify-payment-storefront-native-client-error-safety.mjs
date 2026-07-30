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

function functionBody(source, functionName) {
  const match = new RegExp(`pub\\s+async\\s+fn\\s+${functionName}\\s*\\(`).exec(source);
  if (!match) {
    failures.push(`missing function ${functionName}`);
    return "";
  }
  const openBrace = source.indexOf("{", match.index);
  if (openBrace < 0) {
    failures.push(`missing body for ${functionName}`);
    return "";
  }
  let depth = 0;
  for (let index = openBrace; index < source.length; index += 1) {
    if (source[index] === "{") depth += 1;
    if (source[index] === "}") {
      depth -= 1;
      if (depth === 0) return source.slice(openBrace, index + 1);
    }
  }
  failures.push(`unterminated body for ${functionName}`);
  return "";
}

const paths = {
  transport: "crates/rustok-payment/storefront/src/transport.rs",
  safety:
    "crates/rustok-payment/storefront/src/transport/native_client_error_safety.rs",
  native: "crates/rustok-payment/storefront/src/transport/native_server_adapter.rs",
  serverFunctions:
    "crates/rustok-payment/storefront/src/transport/native_server_adapter/server_functions.rs",
  graphqlAdapter:
    "crates/rustok-payment/storefront/src/transport/graphql_adapter.rs",
  graphqlSafety:
    "crates/rustok-payment/storefront/src/transport/graphql_error_safety.rs",
  evidence:
    "crates/rustok-payment/contracts/evidence/payment-storefront-native-client-error-safety-source.json",
  review:
    "crates/rustok-payment/contracts/evidence/payment-storefront-native-client-error-safety-source-review.json",
  doc: "crates/rustok-payment/docs/storefront-native-client-error-safety.md",
  paymentPlan: "crates/rustok-payment/docs/implementation-plan.md",
  commercePlan: "crates/rustok-commerce/docs/implementation-plan.md",
  nativeGuard: "scripts/verify/verify-payment-storefront-native-error-safety.mjs",
  graphqlGuard: "scripts/verify/verify-payment-storefront-graphql-error-safety.mjs",
};

const transport = read(paths.transport);
const safety = read(paths.safety);
const native = read(paths.native);
const serverFunctions = read(paths.serverFunctions);
const graphqlAdapter = read(paths.graphqlAdapter);
const graphqlSafety = read(paths.graphqlSafety);
const evidence = JSON.parse(read(paths.evidence));
const review = JSON.parse(read(paths.review));
const doc = read(paths.doc);
const paymentPlan = read(paths.paymentPlan);
const commercePlan = read(paths.commercePlan);
const nativeGuard = read(paths.nativeGuard);
const graphqlGuard = read(paths.graphqlGuard);

for (const marker of [
  "mod graphql_adapter;",
  "mod graphql_error_safety;",
  "mod native_client_error_safety;",
  "mod native_server_adapter;",
  "pub type PaymentFacadeError = UiTransportError;",
  "UiTransportPath::NativeServer",
  "UiTransportPath::Graphql",
]) {
  requireText(transport, marker, `${paths.transport}: preserved transport contract`);
}

requireCount(
  transport,
  "native_client_error_safety::NativeClientErrorContext::",
  3,
  `${paths.transport}: native client contexts`,
);
requireCount(
  transport,
  ".map_err(|error| native_context.map_error(error))",
  3,
  `${paths.transport}: native final mappings`,
);
requireCount(
  transport,
  "graphql_error_safety::GraphqlCallContext::new(",
  3,
  `${paths.transport}: preserved GraphQL contexts`,
);
requireCount(
  transport,
  ".map_err(|error| context.map_error(error))",
  3,
  `${paths.transport}: preserved GraphQL mappings`,
);

for (const [operation, constructor, nativeCall, graphqlCall] of [
  [
    "create_payment_collection",
    "NativeClientErrorContext::create_payment_collection",
    "native_server_adapter::create_payment_collection(native_request)",
    "graphql_adapter::create_payment_collection(request)",
  ],
  [
    "fetch_payment_collection",
    "NativeClientErrorContext::fetch_payment_collection",
    "native_server_adapter::fetch_payment_collection(native_request)",
    "graphql_adapter::fetch_payment_collection(request)",
  ],
  [
    "fetch_refund_summary",
    "NativeClientErrorContext::fetch_refund_summary",
    "native_server_adapter::fetch_refund_summary(native_request)",
    "graphql_adapter::fetch_refund_summary(request)",
  ],
]) {
  const body = functionBody(transport, operation);
  for (const marker of [constructor, nativeCall, graphqlCall]) {
    requireText(body, marker, `${paths.transport}: ${operation}`);
  }
  const contextIndex = body.indexOf(constructor);
  const nativeIndex = body.indexOf(nativeCall);
  if (contextIndex < 0 || nativeIndex < 0 || contextIndex > nativeIndex) {
    failures.push(`${paths.transport}: ${operation} native context must precede call`);
  }
}

for (const marker of [
  'const PAYMENT_STOREFRONT_NATIVE_CLIENT_OWNER: &str = "rustok_payment.storefront";',
  '"payment_storefront_native_client_transport"',
  '"Payment storefront request could not be completed"',
  "pub(super) struct NativeClientErrorContext",
  'operation: "create_storefront_payment_collection"',
  'operation: "read_storefront_payment_collection"',
  'operation: "read_storefront_order_refunds"',
  "PaymentTransportError::Validation(message) =>",
  "PaymentTransportError::Validation(message)",
  "raw_error = ?error",
  "owner_operation = self.operation",
  "correlation_id = %self.correlation_id",
  "cart_id_present = self.cart_id_length.is_some()",
  "order_id_present = self.order_id_length.is_some()",
  "command_metadata_present = self.command_metadata_present",
  'code = "payment.storefront_native_client_transport_failed"',
  "boundary = PAYMENT_STOREFRONT_NATIVE_CLIENT_BOUNDARY",
  "PaymentTransportError::ServerFn(",
]) {
  requireText(safety, marker, `${paths.safety}: safe final mapping`);
}

for (const forbidden of [
  "cart_id = %",
  "cart_id = ?",
  "order_id = %",
  "order_id = ?",
  "metadata = %",
  "metadata = ?",
  "source_module =",
  "source_surface =",
  "command =",
  "owner_module =",
  "request = ?",
  "error.to_string()",
]) {
  forbidText(safety, forbidden, `${paths.safety}: raw request or error text`);
}

for (const marker of [
  "pub async fn create_payment_collection(",
  "pub async fn fetch_payment_collection(",
  "pub async fn fetch_refund_summary(",
]) {
  requireText(native, marker, `${paths.native}: preserved adapter call`);
}
requireCount(
  serverFunctions,
  "PaymentTransportError::ServerFn(error.to_string())",
  3,
  `${paths.serverFunctions}: preserved compatibility handoffs`,
);
for (const endpoint of [
  'endpoint = "payment/refund-summary"',
  'endpoint = "payment/payment-collection"',
  'endpoint = "payment/create-payment-collection"',
]) {
  requireText(serverFunctions, endpoint, `${paths.serverFunctions}: preserved endpoint`);
}
requireText(
  graphqlAdapter,
  "PaymentTransportError::Graphql(error.to_string())",
  `${paths.graphqlAdapter}: preserved GraphQL handoff`,
);
requireText(
  graphqlSafety,
  "pub(super) struct GraphqlCallContext",
  `${paths.graphqlSafety}: preserved GraphQL policy`,
);
requireText(
  nativeGuard,
  "PAYMENT_STOREFRONT_NATIVE_OWNER",
  `${paths.nativeGuard}: prior native guard remains registered`,
);
requireText(
  graphqlGuard,
  "GraphqlCallContext::new(",
  `${paths.graphqlGuard}: prior GraphQL guard remains registered`,
);

if (evidence.status !== "payment_storefront_native_client_error_safety_source_unvalidated") {
  failures.push(`${paths.evidence}: unexpected status ${evidence.status}`);
}
if (
  review.status !==
  "payment_storefront_native_client_error_safety_source_reviewed_unvalidated"
) {
  failures.push(`${paths.review}: unexpected status ${review.status}`);
}

for (const [key, expected] of Object.entries({
  operation_count: 3,
  transport_selection_changed: false,
  graphql_transport_changed: false,
  native_adapter_changed: false,
  server_functions_changed: false,
  request_response_dto_changed: false,
  payment_facade_error_contract_preserved: true,
  validation_message_preserved: true,
  technical_native_error_public: false,
  static_technical_public_message: true,
  context_created_before_native_call: true,
  original_error_logged_privately: true,
  per_call_correlation_logging: true,
  safe_request_shape_only: true,
  cart_id_values_logged: false,
  order_id_values_logged: false,
  command_metadata_values_logged: false,
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
  "Payment storefront request could not be completed",
  `${paths.doc}: static technical public message`,
);
requireText(
  paymentPlan,
  "Payment storefront native client error safety: `source_ready_unvalidated`",
  `${paths.paymentPlan}: local source status`,
);
requireText(
  commercePlan,
  "Payment storefront native client error safety: `source_ready_unvalidated`",
  `${paths.commercePlan}: umbrella source status`,
);
requireText(
  commercePlan,
  "Finish correlation-safe mapper cleanup",
  `${paths.commercePlan}: broad ecommerce cleanup remains open`,
);

if (failures.length > 0) {
  console.error("Payment storefront native client error-safety verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  "Payment storefront native client technical failures use a correlation-safe static envelope while validation and GraphQL contracts remain unchanged; execution evidence remains open",
);

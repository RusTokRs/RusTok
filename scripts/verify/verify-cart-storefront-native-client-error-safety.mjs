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
  transport: "crates/rustok-cart/storefront/src/transport/mod.rs",
  safety: "crates/rustok-cart/storefront/src/transport/native_client_error_safety.rs",
  native: "crates/rustok-cart/storefront/src/transport/native_server_adapter.rs",
  nativeSsr: "crates/rustok-cart/storefront/src/transport/native_server_adapter_ssr.rs",
  graphqlSafety: "crates/rustok-cart/storefront/src/transport/graphql_error_safety.rs",
  evidence:
    "crates/rustok-cart/contracts/evidence/storefront-native-client-error-safety-source.json",
  review:
    "crates/rustok-cart/contracts/evidence/storefront-native-client-error-safety-source-review.json",
  doc: "crates/rustok-cart/docs/storefront-native-client-error-safety.md",
  plan: "crates/rustok-cart/docs/implementation-plan.md",
  nativeGuard: "scripts/verify/verify-cart-storefront-native-error-safety.mjs",
  graphqlGuard: "scripts/verify/verify-cart-storefront-graphql-error-safety.mjs",
};

const transport = read(paths.transport);
const safety = read(paths.safety);
const native = read(paths.native);
const nativeSsr = read(paths.nativeSsr);
const graphqlSafety = read(paths.graphqlSafety);
const evidence = JSON.parse(read(paths.evidence));
const review = JSON.parse(read(paths.review));
const doc = read(paths.doc);
const plan = read(paths.plan);
const nativeGuard = read(paths.nativeGuard);
const graphqlGuard = read(paths.graphqlGuard);

requireText(
  transport,
  "mod native_client_error_safety;",
  `${paths.transport}: native client safety module wiring`,
);
requireText(
  transport,
  "pub type CartTransportError = UiTransportError;",
  `${paths.transport}: public transport error alias`,
);
requireText(
  transport,
  "pub type TransportResult<T> = UiTransportResult<T>;",
  `${paths.transport}: public result alias`,
);
requireText(
  transport,
  "UiTransportPath::NativeServer",
  `${paths.transport}: native transport selection`,
);
requireText(
  transport,
  "UiTransportPath::Graphql",
  `${paths.transport}: GraphQL transport selection`,
);
forbidText(
  transport,
  "fallback_attempted = true",
  `${paths.transport}: no fallback may be introduced`,
);

requireCount(
  transport,
  "native_client_error_safety::NativeClientErrorContext::",
  3,
  `${paths.transport}: native client contexts`,
);
requireCount(
  transport,
  "graphql_error_safety::GraphqlCallContext::",
  3,
  `${paths.transport}: preserved GraphQL contexts`,
);
requireCount(
  transport,
  ".map_err(|error| context.map_error(error))",
  6,
  `${paths.transport}: three native and three GraphQL final mappings`,
);

for (const [operation, nativeContext, nativeCall, graphqlContext, graphqlCall] of [
  [
    "fetch_cart",
    "NativeClientErrorContext::fetch_cart",
    "native_server_adapter::fetch_cart(native_request)",
    "GraphqlCallContext::fetch_cart",
    "graphql_adapter::fetch_cart(request)",
  ],
  [
    "decrement_line_item",
    "NativeClientErrorContext::decrement_line_item",
    "native_server_adapter::decrement_line_item(native_request)",
    "GraphqlCallContext::decrement_line_item",
    "graphql_adapter::decrement_line_item(request)",
  ],
  [
    "remove_line_item",
    "NativeClientErrorContext::remove_line_item",
    "native_server_adapter::remove_line_item(native_request)",
    "GraphqlCallContext::remove_line_item",
    "graphql_adapter::remove_line_item(request)",
  ],
]) {
  const body = functionBody(transport, operation);
  for (const marker of [nativeContext, nativeCall, graphqlContext, graphqlCall]) {
    requireText(body, marker, `${paths.transport}: ${operation}`);
  }
  const nativeContextIndex = body.indexOf(nativeContext);
  const nativeCallIndex = body.indexOf(nativeCall);
  const graphqlContextIndex = body.indexOf(graphqlContext);
  const graphqlCallIndex = body.indexOf(graphqlCall);
  if (nativeContextIndex < 0 || nativeContextIndex > nativeCallIndex) {
    failures.push(`${paths.transport}: ${operation} native context must precede call`);
  }
  if (graphqlContextIndex < 0 || graphqlContextIndex > graphqlCallIndex) {
    failures.push(`${paths.transport}: ${operation} GraphQL context must precede call`);
  }
}

for (const marker of [
  'const CART_STOREFRONT_NATIVE_CLIENT_OWNER: &str = "rustok_cart.storefront";',
  '"cart_storefront_native_client_transport"',
  '"Cart storefront request could not be completed"',
  "pub(super) struct NativeClientErrorContext",
  'operation: "fetch_cart"',
  '"decrement_line_item"',
  '"remove_line_item"',
  "ApiError::Validation(message) => ApiError::Validation(message)",
  "raw_error = ?error",
  "owner_operation = self.operation",
  "correlation_id = %self.correlation_id",
  "selected_cart_id_present = self.selected_cart_id_length.is_some()",
  "locale_present = self.locale_length.is_some()",
  "cart_id_present = self.cart_id_length.is_some()",
  "line_item_id_present = self.line_item_id_length.is_some()",
  'code = "cart.storefront_native_client_transport_failed"',
  "boundary = CART_STOREFRONT_NATIVE_CLIENT_BOUNDARY",
  "ApiError::ServerFn(CART_STOREFRONT_NATIVE_CLIENT_PUBLIC_MESSAGE.to_string())",
]) {
  requireText(safety, marker, `${paths.safety}: safe final mapping`);
}

for (const forbidden of [
  "selected_cart_id = %",
  "selected_cart_id = ?",
  "locale = %",
  "locale = ?",
  "cart_id = %",
  "cart_id = ?",
  "line_item_id = %",
  "line_item_id = ?",
  "request = ?",
  "error.to_string()",
]) {
  forbidText(safety, forbidden, `${paths.safety}: raw request or error text`);
}

for (const adapter of [native, nativeSsr]) {
  for (const marker of [
    "pub enum ApiError",
    "Graphql(String)",
    "ServerFn(String)",
    "Validation(String)",
    "pub async fn fetch_cart(",
    "pub async fn decrement_line_item(",
    "pub async fn remove_line_item(",
  ]) {
    requireText(adapter, marker, "native adapter contract");
  }
}
requireText(
  nativeSsr,
  "CART_STOREFRONT_NATIVE_BOUNDARY",
  `${paths.nativeSsr}: mounted server-side safe boundary`,
);
requireText(
  graphqlSafety,
  "pub(super) struct GraphqlCallContext",
  `${paths.graphqlSafety}: GraphQL safety policy`,
);
requireText(
  nativeGuard,
  "Cart storefront native error-safety source invariants passed",
  `${paths.nativeGuard}: prior mounted native guard`,
);
requireText(
  graphqlGuard,
  "Cart storefront GraphQL error-safety source invariants passed",
  `${paths.graphqlGuard}: prior GraphQL guard`,
);

if (evidence.status !== "storefront_native_client_error_safety_source_unvalidated") {
  failures.push(`${paths.evidence}: unexpected status ${evidence.status}`);
}
if (
  review.status !==
  "storefront_native_client_error_safety_source_reviewed_unvalidated"
) {
  failures.push(`${paths.review}: unexpected status ${review.status}`);
}

for (const [key, expected] of Object.entries({
  operation_count: 3,
  transport_selection_changed: false,
  graphql_transport_changed: false,
  native_adapters_changed: false,
  server_functions_changed: false,
  request_response_dto_changed: false,
  ui_transport_error_contract_preserved: true,
  validation_message_preserved: true,
  technical_native_error_public: false,
  static_technical_public_message: true,
  context_created_before_native_call: true,
  original_error_logged_privately: true,
  per_call_correlation_logging: true,
  safe_request_shape_only: true,
  cart_id_values_logged: false,
  line_item_id_values_logged: false,
  locale_values_logged: false,
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
  "Cart storefront request could not be completed",
  `${paths.doc}: static technical public message`,
);
requireText(
  plan,
  "No verification command was executed in this source wave.",
  `${paths.plan}: execution disclosure remains explicit`,
);

if (failures.length > 0) {
  console.error("Cart storefront native client error-safety verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  "Cart storefront native client technical failures use a correlation-safe static envelope while validation and GraphQL contracts remain unchanged; execution evidence remains open",
);

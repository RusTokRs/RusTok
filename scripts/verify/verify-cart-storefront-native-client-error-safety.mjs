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
  let depth = 0;
  for (let index = openBrace; index >= 0 && index < source.length; index += 1) {
    if (source[index] === "{") depth += 1;
    if (source[index] === "}") {
      depth -= 1;
      if (depth === 0) return source.slice(openBrace, index + 1);
    }
  }
  failures.push(`unterminated body for ${functionName}`);
  return "";
}

const files = {
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

const transport = read(files.transport);
const safety = read(files.safety);
const native = read(files.native);
const nativeSsr = read(files.nativeSsr);
const graphqlSafety = read(files.graphqlSafety);
const evidence = JSON.parse(read(files.evidence));
const review = JSON.parse(read(files.review));
const doc = read(files.doc);
const plan = read(files.plan);
const nativeGuard = read(files.nativeGuard);
const graphqlGuard = read(files.graphqlGuard);

for (const marker of [
  "mod native_client_error_safety;",
  "pub type CartTransportError = UiTransportError;",
  "pub type TransportResult<T> = UiTransportResult<T>;",
  "UiTransportPath::NativeServer",
  "UiTransportPath::Graphql",
]) requireText(transport, marker, `${files.transport}: preserved transport contract`);
forbidText(transport, "fallback_attempted = true", `${files.transport}: fallback policy`);
requireCount(
  transport,
  "native_client_error_safety::NativeClientErrorContext::",
  3,
  `${files.transport}: native contexts`,
);
requireCount(
  transport,
  "graphql_error_safety::GraphqlCallContext::",
  3,
  `${files.transport}: GraphQL contexts`,
);
requireCount(
  transport,
  ".map_err(|error| context.map_error(error))",
  6,
  `${files.transport}: native plus GraphQL final mappings`,
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
    requireText(body, marker, `${files.transport}: ${operation}`);
  }
  if (body.indexOf(nativeContext) > body.indexOf(nativeCall)) {
    failures.push(`${files.transport}: ${operation} native context must precede call`);
  }
  if (body.indexOf(graphqlContext) > body.indexOf(graphqlCall)) {
    failures.push(`${files.transport}: ${operation} GraphQL context must precede call`);
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
]) requireText(safety, marker, `${files.safety}: safe final mapping`);

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
]) forbidText(safety, forbidden, `${files.safety}: raw request or error text`);

for (const adapter of [native, nativeSsr]) {
  for (const marker of [
    "pub enum ApiError",
    "Graphql(String)",
    "ServerFn(String)",
    "Validation(String)",
    "pub async fn fetch_cart(",
    "pub async fn decrement_line_item(",
    "pub async fn remove_line_item(",
  ]) requireText(adapter, marker, "native adapter contract");
}
requireText(
  nativeSsr,
  "CART_STOREFRONT_NATIVE_BOUNDARY",
  `${files.nativeSsr}: mounted safe boundary`,
);
requireText(
  graphqlSafety,
  "pub(super) struct GraphqlCallContext",
  `${files.graphqlSafety}: GraphQL safety policy`,
);
requireText(
  nativeGuard,
  "Cart storefront native error-safety verification failed:",
  `${files.nativeGuard}: mounted native guard identity`,
);
requireText(
  graphqlGuard,
  "Cart storefront GraphQL error-safety verification failed:",
  `${files.graphqlGuard}: GraphQL guard identity`,
);

if (evidence.status !== "storefront_native_client_error_safety_source_unvalidated") {
  failures.push(`${files.evidence}: unexpected status ${evidence.status}`);
}
if (
  review.status !==
  "storefront_native_client_error_safety_source_reviewed_unvalidated"
) {
  failures.push(`${files.review}: unexpected status ${review.status}`);
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
    failures.push(`${files.evidence}: source_contract.${key} must be ${expected}`);
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
    failures.push(`${files.evidence}: validation.${key} must remain false`);
  }
}
requireText(doc, "Status: **source-ready / unvalidated**", `${files.doc}: status`);
requireText(
  doc,
  "Cart storefront request could not be completed",
  `${files.doc}: static technical message`,
);
requireText(
  plan,
  "Storefront native client error safety: `source_ready_unvalidated`",
  `${files.plan}: local source status`,
);
requireText(
  plan,
  "No verification command was executed in this source wave.",
  `${files.plan}: execution disclosure`,
);

if (failures.length > 0) {
  console.error("Cart storefront native client error-safety verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  "Cart storefront native client technical failures use a correlation-safe static envelope while validation and GraphQL contracts remain unchanged; execution evidence remains open",
);

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
  transport: "crates/rustok-order/storefront/src/transport.rs",
  adapter: "crates/rustok-order/storefront/src/transport/native_server_adapter.rs",
  context:
    "crates/rustok-order/storefront/src/transport/native_server_adapter/native_client_error_safety.rs",
  serverFunctions:
    "crates/rustok-order/storefront/src/transport/native_server_adapter/server_functions.rs",
  graphqlSafety:
    "crates/rustok-order/storefront/src/transport/graphql_error_safety.rs",
  evidence:
    "crates/rustok-order/contracts/evidence/storefront-native-client-diagnostics-source.json",
  review:
    "crates/rustok-order/contracts/evidence/storefront-native-client-diagnostics-source-review.json",
  doc: "crates/rustok-order/docs/storefront-native-client-diagnostics.md",
  commercePlan: "crates/rustok-commerce/docs/implementation-plan.md",
  runtimeGuard: "scripts/verify/verify-order-storefront-runtime-error-diagnostics.mjs",
  graphqlGuard: "scripts/verify/verify-order-storefront-graphql-error-safety.mjs",
};

const transport = read(paths.transport);
const adapter = read(paths.adapter);
const context = read(paths.context);
const serverFunctions = read(paths.serverFunctions);
const graphqlSafety = read(paths.graphqlSafety);
const evidence = JSON.parse(read(paths.evidence));
const review = JSON.parse(read(paths.review));
const doc = read(paths.doc);
const commercePlan = read(paths.commercePlan);
const runtimeGuard = read(paths.runtimeGuard);
const graphqlGuard = read(paths.graphqlGuard);

for (const marker of [
  "mod graphql_adapter;",
  "mod graphql_error_safety;",
  "mod native_server_adapter;",
  "move || native_server_adapter::complete_checkout(native_request)",
  "let context = graphql_error_safety::GraphqlCallContext::new(&request);",
  "graphql_adapter::complete_checkout(request)",
  ".map_err(|error| context.map_error(error))",
  "UiTransportPath::NativeServer",
  "UiTransportPath::Graphql",
]) {
  requireText(transport, marker, `${paths.transport}: preserved storefront facade`);
}
for (const forbidden of ["fallback_failed(", "fallback_attempted = true"]) {
  forbidText(transport, forbidden, `${paths.transport}: no fallback`);
}

for (const marker of [
  "mod native_client_error_safety;",
  "mod server_functions;",
  "server_functions::complete_checkout_server(request).await",
]) {
  requireText(adapter, marker, `${paths.adapter}: native adapter wiring`);
}

for (const marker of [
  "use super::native_client_error_safety::NativeClientDiagnosticContext;",
  "let context = NativeClientDiagnosticContext::new(&request);",
  "storefront_order_complete_checkout_native(request)",
  "context.record_error(&error);",
  "CheckoutCompletionTransportError::ServerFn(",
  '"Checkout transport is temporarily unavailable".to_string()',
]) {
  requireText(serverFunctions, marker, `${paths.serverFunctions}: final native mapping`);
}

const contextIndex = serverFunctions.indexOf("NativeClientDiagnosticContext::new(&request)");
const callIndex = serverFunctions.indexOf("storefront_order_complete_checkout_native(request)");
const diagnosticIndex = serverFunctions.indexOf("context.record_error(&error)");
const publicIndex = serverFunctions.indexOf(
  '"Checkout transport is temporarily unavailable".to_string()',
);
if (
  [contextIndex, callIndex, diagnosticIndex, publicIndex].some((value) => value < 0) ||
  !(contextIndex < callIndex && callIndex < diagnosticIndex && diagnosticIndex < publicIndex)
) {
  failures.push(
    `${paths.serverFunctions}: expected context -> generated call -> diagnostic -> static public mapping order`,
  );
}
requireCount(
  serverFunctions,
  '"Checkout transport is temporarily unavailable".to_string()',
  1,
  `${paths.serverFunctions}: one preserved public transport message`,
);

for (const marker of [
  'const ORDER_STOREFRONT_NATIVE_CLIENT_OWNER: &str = "rustok_order.storefront";',
  'const ORDER_STOREFRONT_NATIVE_CLIENT_OPERATION: &str = "complete_storefront_checkout";',
  '"order_storefront_native_client_transport"',
  "pub(super) struct NativeClientDiagnosticContext",
  "Uuid::new_v4()",
  '"order-storefront-native-client:{}:{}"',
  "cart_id_length: request.cart_id.chars().count()",
  "idempotency_key_length: request.idempotency_key.chars().count()",
  "source_module_length: request.metadata.source_module.chars().count()",
  "source_surface_length: request.metadata.source_surface.chars().count()",
  "command_length: request.metadata.command.chars().count()",
  "owner_module_length: request.metadata.owner_module.chars().count()",
  "raw_error = ?error",
  "owner = ORDER_STOREFRONT_NATIVE_CLIENT_OWNER",
  "owner_operation = ORDER_STOREFRONT_NATIVE_CLIENT_OPERATION",
  "correlation_id = %self.correlation_id",
  "cart_id_length = self.cart_id_length",
  "idempotency_key_length = self.idempotency_key_length",
  "source_module_length = self.source_module_length",
  "source_surface_length = self.source_surface_length",
  "command_length = self.command_length",
  "owner_module_length = self.owner_module_length",
  "command_metadata_present = true",
  'code = "order.storefront_native_client_transport_failed"',
  "boundary = ORDER_STOREFRONT_NATIVE_CLIENT_BOUNDARY",
]) {
  requireText(context, marker, `${paths.context}: correlation-safe diagnostic shape`);
}

for (const forbidden of [
  "cart_id = %",
  "cart_id = ?",
  "idempotency_key = %",
  "idempotency_key = ?",
  "source_module = %",
  "source_module = ?",
  "source_surface = %",
  "source_surface = ?",
  "command = %",
  "command = ?",
  "owner_module = %",
  "owner_module = ?",
  "metadata = ?",
  "request = ?",
  "error.to_string()",
]) {
  forbidText(context, forbidden, `${paths.context}: raw request or error text`);
}

for (const marker of [
  'endpoint = "order/complete-checkout"',
  "shared_get::<TransactionalEventBus>()",
  "shared_get::<PaymentProviderRegistry>()",
  "shared_get::<ProductCatalogReadRuntime>()",
  "extract::<rustok_api::RequestContext>()",
  "extract::<rustok_api::TenantContext>()",
  "extract::<rustok_api::OptionalAuthContext>()",
  'ServerFnError::new("Checkout request is invalid")',
  "StorefrontCheckoutCompletionCommand {",
  '"source_module": metadata.source_module',
  '"source_surface": metadata.source_surface',
  '"command": metadata.command',
  '"owner_module": metadata.owner_module',
  '"create_fulfillment": metadata.create_fulfillment',
  "native_checkout_runtime_error(&request_context, tenant.id, correlation_id, error)",
  'ServerFnError::new(format!("{public_code}: {public_message}"))',
]) {
  requireText(serverFunctions, marker, `${paths.serverFunctions}: mounted runtime contract`);
}
requireCount(
  serverFunctions,
  'ServerFnError::new("Checkout request is invalid")',
  2,
  `${paths.serverFunctions}: preserved validation envelopes`,
);

requireText(
  graphqlSafety,
  "pub(super) struct GraphqlCallContext",
  `${paths.graphqlSafety}: preserved GraphQL policy`,
);
requireText(
  runtimeGuard,
  "Order storefront runtime-error diagnostics verification failed",
  `${paths.runtimeGuard}: existing SSR runtime guard`,
);
requireText(
  graphqlGuard,
  "Order storefront GraphQL error-safety verification failed",
  `${paths.graphqlGuard}: existing GraphQL guard`,
);

if (evidence.status !== "order_storefront_native_client_diagnostics_source_unvalidated") {
  failures.push(`${paths.evidence}: unexpected status ${evidence.status}`);
}
if (
  review.status !==
  "order_storefront_native_client_diagnostics_source_reviewed_unvalidated"
) {
  failures.push(`${paths.review}: unexpected status ${review.status}`);
}

for (const [key, expected] of Object.entries({
  operation_count: 1,
  context_created_before_server_function_call: true,
  original_error_logged_privately: true,
  per_call_correlation_logging: true,
  safe_request_shape_only: true,
  cart_id_values_logged: false,
  idempotency_key_values_logged: false,
  command_metadata_values_logged: false,
  static_public_transport_message_preserved: true,
  public_transport_variant_preserved: true,
  transport_facade_changed: false,
  graphql_transport_changed: false,
  request_response_dto_changed: false,
  mounted_endpoint_changed: false,
  runtime_dependency_composition_changed: false,
  runtime_error_mapper_changed: false,
  checkout_command_payload_changed: false,
  validation_messages_changed: false,
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
  "remote_transport_proven",
]) {
  if (evidence.validation?.[key] !== false) {
    failures.push(`${paths.evidence}: validation.${key} must remain false`);
  }
}

requireText(doc, "Status: **source-ready / unvalidated**", `${paths.doc}: source status`);
requireText(
  doc,
  "Checkout transport is temporarily unavailable",
  `${paths.doc}: preserved public message`,
);
requireText(
  commercePlan,
  "Finish correlation-safe mapper cleanup",
  `${paths.commercePlan}: broad ecommerce cleanup remains open`,
);

if (failures.length > 0) {
  console.error("Order storefront native client diagnostics verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  "Order storefront native client failures retain the static public envelope and use correlation-safe request-shape diagnostics; execution evidence remains open",
);

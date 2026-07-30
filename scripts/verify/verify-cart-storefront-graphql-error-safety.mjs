#!/usr/bin/env node

import { existsSync, readFileSync } from "node:fs";
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

const files = {
  transport: "crates/rustok-cart/storefront/src/transport/mod.rs",
  adapter: "crates/rustok-cart/storefront/src/transport/graphql_adapter.rs",
  safety: "crates/rustok-cart/storefront/src/transport/graphql_error_safety.rs",
  nativeSafety: "crates/rustok-cart/storefront/src/transport/native_server_adapter_ssr.rs",
  evidence: "crates/rustok-cart/contracts/evidence/storefront-graphql-error-safety-source.json",
  review: "crates/rustok-cart/contracts/evidence/storefront-graphql-error-safety-source-review.json",
};

for (const filePath of Object.values(files)) {
  if (!existsSync(path.join(root, filePath))) {
    failures.push(`${filePath}: expected cart storefront GraphQL safety file`);
  }
}

const transport = read(files.transport);
const adapter = read(files.adapter);
const safety = read(files.safety);
const nativeSafety = read(files.nativeSafety);
const evidence = JSON.parse(read(files.evidence));
const review = JSON.parse(read(files.review));

for (const [value, label] of [
  ["mod graphql_error_safety;", "safety module wiring"],
  ["GraphqlCallContext::fetch_cart(&request)", "fetch context"],
  ["GraphqlCallContext::decrement_line_item(&request)", "decrement context"],
  ["GraphqlCallContext::remove_line_item(&request)", "remove context"],
  ["graphql_adapter::fetch_cart(request)", "fetch adapter preservation"],
  ["graphql_adapter::decrement_line_item(request)", "decrement adapter preservation"],
  ["graphql_adapter::remove_line_item(request)", "remove adapter preservation"],
  [".map_err(|error| context.map_error(error))", "consumer-boundary mapping"],
  ["NativeClientErrorContext::fetch_cart(&native_request)", "native fetch context preservation"],
  ["native_server_adapter::fetch_cart(native_request)", "native fetch preservation"],
  [
    "NativeClientErrorContext::decrement_line_item(",
    "native decrement context preservation",
  ],
  [
    "native_server_adapter::decrement_line_item(native_request)",
    "native decrement preservation",
  ],
  ["NativeClientErrorContext::remove_line_item(", "native remove context preservation"],
  ["native_server_adapter::remove_line_item(native_request)", "native remove preservation"],
]) requireText(transport, value, label);
forbidText(transport, "move || graphql_adapter::", "unsanitized GraphQL closure");

for (const [value, label] of [
  ["Self::Graphql(value.to_string())", "typed display handoff"],
  ["STOREFRONT_CART_QUERY", "cart query preservation"],
  ["UPDATE_STOREFRONT_CART_LINE_ITEM_MUTATION", "quantity mutation preservation"],
  ["REMOVE_STOREFRONT_CART_LINE_ITEM_MUTATION", "remove mutation preservation"],
  ["parse_cart_id(request.selected_cart_id)?", "fetch validation preservation"],
  ["parse_line_item_id(line_item_id)?", "line-item validation preservation"],
]) requireText(adapter, value, label);

for (const [value, label] of [
  ["GraphqlHttpError::from_str", "typed GraphQL display reparse"],
  ["let ApiError::Graphql(raw_error) = error else", "GraphQL-only mapping"],
  ["return error;", "non-GraphQL pass-through"],
  ["GraphqlHttpError::Network", "network policy"],
  ["GraphqlHttpError::Http(_)", "HTTP policy"],
  ["GraphqlHttpError::Unauthorized", "unauthorized policy"],
  ["GraphqlHttpError::Graphql(_)", "GraphQL rejection policy"],
  ["cart.storefront_graphql_network_unavailable", "network code"],
  ["cart.storefront_graphql_http_unavailable", "HTTP code"],
  ["cart.storefront_graphql_authentication_required", "authentication code"],
  ["cart.storefront_graphql_request_rejected", "rejection code"],
  ["cart.storefront_graphql_unknown_failure", "unknown code"],
  ["Cart storefront is temporarily unavailable", "unavailable public envelope"],
  ["Cart authentication is required", "authentication public envelope"],
  ["Cart request could not be completed", "request public envelope"],
  ["Uuid::new_v4()", "unique correlation id"],
  ["owner_operation = self.owner_operation", "owner operation diagnostics"],
  ["correlation_id = %self.correlation_id", "correlation diagnostics"],
  ["tenant_slug_length", "safe tenant fact"],
  ["selected_cart_id_length", "safe selected-cart fact"],
  ["locale_length", "safe locale fact"],
  ["cart_id_length", "safe cart fact"],
  ["line_item_id_length", "safe line-item fact"],
  ["command_kind", "safe command fact"],
  ["raw_error = %raw_error", "private raw cause diagnostics"],
  ["ApiError::Graphql(public_message.to_string())", "static public remap"],
]) requireText(safety, value, label);

for (const forbidden of [
  "tenant_slug = %",
  "selected_cart_id = %",
  "locale = %",
  "cart_id = %",
  "line_item_id = %",
  "graphql_url =",
  "query = %",
  "variables =",
  "token =",
]) forbidText(safety, forbidden, "safe GraphQL diagnostics");

for (const [value, label] of [
  ["const CART_STOREFRONT_NATIVE_BOUNDARY", "native safety boundary"],
  ["fn tenant_context_error", "native tenant mapper"],
  ["fn cart_error", "native cart mapper"],
]) requireText(nativeSafety, value, label);

if (evidence.status !== "cart_storefront_graphql_error_safety_source_unvalidated") {
  failures.push(`evidence status mismatch: ${evidence.status}`);
}
if (review.status !== "cart_storefront_graphql_error_safety_source_reviewed_unvalidated") {
  failures.push(`review status mismatch: ${review.status}`);
}
for (const [key, expected] of Object.entries({
  public_consumer_boundary_sanitized: true,
  private_graphql_adapter_changed: false,
  graphql_documents_changed: false,
  request_response_dto_changed: false,
  validation_changed: false,
  native_server_functions_changed: false,
  transport_selection_changed: false,
  native_to_graphql_fallback_added: false,
  graphql_http_error_reparsed: true,
  correlation_logging: true,
  safe_request_shape_logging: true,
  raw_request_identifiers_logged: false,
  raw_tenant_slug_logged: false,
  raw_graphql_error_public: false,
  non_graphql_errors_pass_through: true,
})) {
  if (evidence.source_contract?.[key] !== expected) {
    failures.push(`evidence source_contract.${key} must be ${expected}`);
  }
}
for (const key of [
  "tests_run",
  "cargo_run",
  "format_run",
  "verifiers_run",
  "workflow_checks_run",
  "ci_run",
  "graphql_runtime_proven",
  "browser_runtime_proven",
  "mounted_parity_proven",
  "production_proven",
]) {
  if (evidence.validation?.[key] !== false) {
    failures.push(`evidence validation.${key} must remain false`);
  }
}

if (failures.length > 0) {
  console.error("Cart storefront GraphQL error-safety verification failed:");
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  "✔ cart storefront GraphQL failures use static public envelopes with correlation-aware private diagnostics; source evidence remains unvalidated",
);

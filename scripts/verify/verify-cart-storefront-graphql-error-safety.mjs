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
  doc: "crates/rustok-cart/docs/storefront-graphql-error-safety.md",
  masterPlan: "crates/rustok-commerce/docs/implementation-plan.md",
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
const doc = read(files.doc);
const masterPlan = read(files.masterPlan);

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
  ["NativeClientErrorContext::decrement_line_item(", "native decrement context preservation"],
  ["native_server_adapter::decrement_line_item(native_request)", "native decrement preservation"],
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
  ["pub(super) struct GraphqlCallContext", "private call context"],
  ["let ApiError::Graphql(raw_error) = error else", "GraphQL-only mapping"],
  ["return error;", "non-GraphQL pass-through"],
  ["let raw_error_present = !raw_error.trim().is_empty();", "raw display presence fact"],
  ["let raw_error_length = raw_error.chars().count();", "raw display length fact"],
  ["GraphqlHttpError::from_str(raw_error.as_str())", "typed GraphQL display reparse"],
  ["let parsed_error_valid = parsed_error.is_ok();", "typed parse validity fact"],
  ["GraphqlHttpError::Network", "network policy"],
  ["GraphqlHttpError::Http(_)", "HTTP policy"],
  ["GraphqlHttpError::Unauthorized", "unauthorized policy"],
  ["GraphqlHttpError::Graphql(_)", "GraphQL rejection policy"],
  ['"network"', "closed network category"],
  ['"http"', "closed HTTP category"],
  ['"unauthorized"', "closed authentication category"],
  ['"graphql"', "closed GraphQL category"],
  ['"unknown"', "closed unknown category"],
  ["cart.storefront_graphql_network_unavailable", "network code"],
  ["cart.storefront_graphql_http_unavailable", "HTTP code"],
  ["cart.storefront_graphql_authentication_required", "authentication code"],
  ["cart.storefront_graphql_request_rejected", "rejection code"],
  ["cart.storefront_graphql_unknown_failure", "unknown code"],
  ["Cart storefront is temporarily unavailable", "unavailable public envelope"],
  ["Cart authentication is required", "authentication public envelope"],
  ["Cart request could not be completed", "request public envelope"],
  ["Uuid::new_v4()", "unique correlation id"],
  ["raw_error_present,", "bounded raw display presence diagnostics"],
  ["raw_error_length,", "bounded raw display length diagnostics"],
  ["parsed_error_valid,", "bounded typed parse diagnostics"],
  ["owner_operation = self.owner_operation", "owner operation diagnostics"],
  ["correlation_id = %self.correlation_id", "correlation diagnostics"],
  ["tenant_slug_length", "safe tenant fact"],
  ["selected_cart_id_length", "safe selected-cart fact"],
  ["locale_length", "safe locale fact"],
  ["cart_id_length", "safe cart fact"],
  ["line_item_id_length", "safe line-item fact"],
  ["command_kind", "safe command fact"],
  ["error_kind,", "closed error category diagnostics"],
  ["code,", "stable code diagnostics"],
  ["ApiError::Graphql(public_message.to_string())", "static public remap"],
]) requireText(safety, value, label);

for (const forbidden of [
  "raw_error = %raw_error",
  "raw_error = ?raw_error",
  "parsed_error = ?parsed_error",
  "parsed_error = %parsed_error",
  "tenant_slug = %",
  "tenant_slug = ?",
  "selected_cart_id = %",
  "selected_cart_id = ?",
  "locale = %",
  "locale = ?",
  "cart_id = %",
  "cart_id = ?",
  "line_item_id = %",
  "line_item_id = ?",
  "graphql_url =",
  "query = %",
  "query = ?",
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
  raw_graphql_detail_logged: false,
  parsed_graphql_error_debug_logged: false,
  raw_graphql_detail_shape_logged: true,
  typed_parse_validity_logged: true,
  closed_graphql_error_category_logged: true,
  non_graphql_errors_pass_through: true,
  broad_ecommerce_cleanup_closed: false,
})) {
  if (evidence.source_contract?.[key] !== expected) {
    failures.push(`evidence source_contract.${key} must be ${expected}`);
  }
}
for (const [key, expected] of Object.entries({
  typed_graphql_variant_policy: true,
  static_public_graphql_messages: true,
  raw_graphql_detail_logging_removed: true,
  parsed_graphql_error_debug_logging_removed: true,
  bounded_graphql_error_shape_retained: true,
  all_three_operations_preserved: true,
  per_call_correlation_id: true,
  safe_request_shape_only: true,
  private_graphql_adapter_changed: false,
  native_path_changed: false,
  graphql_documents_changed: false,
  transport_selection_changed: false,
  runtime_evidence_claimed: false,
})) {
  if (review.implementation_review?.[key] !== expected) {
    failures.push(`review implementation_review.${key} must be ${expected}`);
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
for (const marker of [
  "Status: source-unvalidated",
  "Raw GraphQL display text is not written to the event.",
  "Debug output from the parsed typed error is not written to the event.",
  "raw-display presence and character length",
  "The broad ecommerce correlation-safe mapper cleanup remains open.",
  "No tests, verifiers, Cargo commands, formatting, workflows, or CI were run",
]) requireText(doc, marker, `${files.doc}: truthful documentation`);
requireText(
  masterPlan,
  "Finish correlation-safe mapper cleanup",
  `${files.masterPlan}: broad mapper cleanup remains open`,
);

if (failures.length > 0) {
  console.error("Cart storefront GraphQL error-safety verification failed:");
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  "✔ cart storefront GraphQL failures retain bounded error/request shape and static public envelopes; source evidence remains unvalidated",
);

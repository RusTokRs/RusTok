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

const transportPath = "crates/rustok-product/storefront/src/transport/mod.rs";
const policyPath =
  "crates/rustok-product/storefront/src/transport/graphql_error_safety.rs";
const adapterPath =
  "crates/rustok-product/storefront/src/transport/graphql_adapter.rs";
const nativePath =
  "crates/rustok-product/storefront/src/transport/native_server_adapter.rs";
const evidencePath =
  "crates/rustok-product/contracts/evidence/storefront-graphql-error-safety-source.json";

const transport = read(transportPath);
const policy = read(policyPath);
const adapter = read(adapterPath);
const native = read(nativePath);
const evidence = JSON.parse(read(evidencePath));

for (const [value, label] of [
  ["mod graphql_error_safety;", "policy module wiring"],
  ["GraphqlCallContext::fetch_products(&request, &controls)", "catalog operation context"],
  ["GraphqlCallContext::fetch_catalog_search_options(&locale)", "search-options operation context"],
  [".map_err(|error| context.map_error(error))", "facade error mapping"],
  ["catalog_list_native::fetch_products(native_request, native_controls)", "native catalog path preservation"],
  ["native_server_adapter::fetch_catalog_search_options(native_locale)", "native search-options path preservation"],
]) requireText(transport, value, label);

if ((transport.match(/\.map_err\(\|error\| context\.map_error\(error\)\)/g) || []).length !== 2) {
  failures.push(`${transportPath}: expected exactly two GraphQL context mappings`);
}

for (const [value, label] of [
  ["pub(super) struct GraphqlCallContext", "private call context"],
  ["pub(super) fn fetch_products", "catalog context constructor"],
  ["pub(super) fn fetch_catalog_search_options", "search-options context constructor"],
  ["GraphqlHttpError::from_str", "typed display reparse"],
  ["let ApiError::Graphql(raw_error) = error else", "GraphQL-only remap"],
  ["return error;", "non-GraphQL pass-through"],
  ["GraphqlHttpError::Network", "network policy"],
  ["GraphqlHttpError::Http(_)", "HTTP policy"],
  ["GraphqlHttpError::Unauthorized", "unauthorized policy"],
  ["GraphqlHttpError::Graphql(_)", "GraphQL rejection policy"],
  ["product.storefront_graphql_network_unavailable", "network code"],
  ["product.storefront_graphql_http_unavailable", "HTTP code"],
  ["product.storefront_graphql_authentication_required", "authentication code"],
  ["product.storefront_graphql_request_rejected", "request rejection code"],
  ["product.storefront_graphql_unknown_failure", "unknown code"],
  ["Product storefront is temporarily unavailable", "temporary public envelope"],
  ["Product storefront authentication is required", "authentication public envelope"],
  ["Product storefront request could not be completed", "rejection public envelope"],
  ["ApiError::Graphql(public_message.to_string())", "static public return"],
  ["Uuid::new_v4()", "unique correlation id"],
  ["owner = PRODUCT_STOREFRONT_GRAPHQL_OWNER", "owner diagnostics"],
  ["owner_operation = self.owner_operation", "operation diagnostics"],
  ["correlation_id = %self.correlation_id", "correlation diagnostics"],
  ["raw_error = %raw_error", "private raw cause diagnostics"],
  ["parsed_error = ?parsed_error", "typed cause diagnostics"],
  ["tenant_slug_length", "safe tenant shape"],
  ["selected_handle_length", "safe handle shape"],
  ["locale_length", "safe locale shape"],
  ["currency_code_length", "safe currency shape"],
  ["region_id_length", "safe region shape"],
  ["price_list_id_length", "safe price-list shape"],
  ["channel_id_length", "safe channel-id shape"],
  ["channel_slug_length", "safe channel-slug shape"],
  ["quantity_present", "safe quantity presence"],
  ["search_length", "safe search shape"],
  ["category_id_length", "safe category shape"],
  ["sort_by_present", "safe sort presence"],
  ["sort_direction_present", "safe direction presence"],
  ["attribute_filter_count", "safe filter count"],
]) requireText(policy, value, label);

for (const value of [
  "selected_handle =",
  "locale = %",
  "currency_code = %",
  "region_id = %",
  "price_list_id = %",
  "channel_id = %",
  "channel_slug = %",
  "quantity =",
  "search = %",
  "category_id = %",
  "attribute_filters =",
  "graphql_url",
  "GraphqlRequest",
]) forbidText(policy, value, "GraphQL safety policy");

for (const [value, label] of [
  ["impl From<rustok_graphql::GraphqlHttpError> for ApiError", "private typed handoff"],
  ["Self::Graphql(value.to_string())", "private display handoff"],
  ["STOREFRONT_PRODUCTS_QUERY", "catalog query preservation"],
  ["STOREFRONT_PRODUCT_QUERY", "detail query preservation"],
  ["STOREFRONT_PRICING_PRODUCT_QUERY", "pricing query preservation"],
  ["STOREFRONT_CATALOG_SEARCH_OPTIONS_QUERY", "search-options query preservation"],
]) requireText(adapter, value, label);

requireText(native, "pub enum ApiError", "shared transport error contract");
requireText(native, "Graphql(String)", "GraphQL error variant preservation");
requireText(native, "ServerFn(String)", "native error variant preservation");

if (evidence.status !== "product_storefront_graphql_error_safety_source_unvalidated") {
  failures.push(`evidence status mismatch: ${evidence.status}`);
}
for (const [key, expected] of Object.entries({
  public_consumer_boundary_sanitized: true,
  private_graphql_adapter_changed: false,
  graphql_documents_changed: false,
  request_response_dto_changed: false,
  catalog_controls_changed: false,
  pricing_context_changed: false,
  native_paths_changed: false,
  transport_selection_changed: false,
  native_to_graphql_fallback_added: false,
  graphql_http_error_reparsed: true,
  correlation_logging: true,
  safe_request_shape_logging: true,
  raw_request_values_logged: false,
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
  console.error("Product storefront GraphQL error-safety verification failed:");
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  "✔ product storefront GraphQL failures use static public envelopes with private correlated diagnostics; source evidence remains unvalidated",
);

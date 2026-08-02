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
const countText = (source, value) => source.split(value).length - 1;

const cargoPath = "crates/rustok-pricing/storefront/Cargo.toml";
const transportPath = "crates/rustok-pricing/storefront/src/transport/mod.rs";
const graphqlPath =
  "crates/rustok-pricing/storefront/src/transport/graphql_adapter.rs";
const safetyPath =
  "crates/rustok-pricing/storefront/src/transport/graphql_error_safety.rs";
const nativePath =
  "crates/rustok-pricing/storefront/src/transport/native_server_adapter.rs";
const corePath = "crates/rustok-pricing/storefront/src/core.rs";
const graphqlHttpPath = "crates/rustok-graphql/src/lib.rs";
const evidencePath =
  "crates/rustok-pricing/contracts/evidence/storefront-graphql-error-safety-source.json";
const reviewPath =
  "crates/rustok-pricing/contracts/evidence/storefront-graphql-error-safety-source-review.json";
const docPath =
  "crates/rustok-pricing/docs/storefront-graphql-error-safety.md";
const masterPlanPath = "crates/rustok-commerce/docs/implementation-plan.md";

const cargo = read(cargoPath);
const transport = read(transportPath);
const graphql = read(graphqlPath);
const safety = read(safetyPath);
const native = read(nativePath);
const core = read(corePath);
const graphqlHttp = read(graphqlHttpPath);
const evidence = JSON.parse(read(evidencePath));
const review = JSON.parse(read(reviewPath));
const doc = read(docPath);
const masterPlan = read(masterPlanPath);

requireText(cargo, "tracing.workspace = true", `${cargoPath}: all-profile tracing dependency`);
forbidText(cargo, '"dep:tracing"', `${cargoPath}: stale SSR-only tracing feature`);
forbidText(
  cargo,
  "tracing = { workspace = true, optional = true }",
  `${cargoPath}: stale optional tracing dependency`,
);
requireText(cargo, "uuid.workspace = true", `${cargoPath}: correlation UUID dependency`);

requireText(
  transport,
  "mod graphql_error_safety;",
  `${transportPath}: private GraphQL policy module`,
);
const fetchStart = transport.indexOf("pub(crate) async fn fetch_storefront_pricing(");
const testsStart = transport.indexOf("#[cfg(test)]", fetchStart);
if (fetchStart < 0 || testsStart < 0) {
  failures.push(`${transportPath}: storefront pricing facade boundaries are missing`);
} else {
  const fetchBlock = transport.slice(fetchStart, testsStart);
  for (const marker of [
    "graphql_error_safety::GraphqlCallContext::new(&query)",
    "graphql_adapter::fetch_storefront_pricing(query)",
    ".map_err(|error| context.map_error(error))",
  ]) requireText(fetchBlock, marker, `${transportPath}: final GraphQL public boundary`);
  const contextIndex = fetchBlock.indexOf("GraphqlCallContext::new(&query)");
  const callIndex = fetchBlock.indexOf("graphql_adapter::fetch_storefront_pricing(query)");
  if (contextIndex < 0 || callIndex < 0 || contextIndex > callIndex) {
    failures.push(`${transportPath}: GraphQL context must be created before the adapter call`);
  }
  forbidText(fetchBlock, "error.to_string()", `${transportPath}: direct public display mapping`);
}

for (const marker of [
  "impl From<rustok_graphql::GraphqlHttpError> for ApiError",
  "Self::Graphql(value.to_string())",
  ".map_err(ApiError::from)",
]) requireText(graphql, marker, `${graphqlPath}: private GraphQL HTTP capture`);
if (countText(graphql, ".map_err(|err| ApiError::ServerFn(err.to_string()))?;") !== 2) {
  failures.push(`${graphqlPath}: exactly two existing pricing validation mappings must remain`);
}
for (const marker of [
  "parse_optional_uuid_string(query.channel_id.clone(), \"channel_id\")",
  "sanitize_resolution_context(",
  "STOREFRONT_PRODUCTS_QUERY",
  "STOREFRONT_PRODUCT_QUERY",
  "try_join_all",
]) requireText(graphql, marker, `${graphqlPath}: preserved request and composition contract`);

for (const [marker, label] of [
  ["pub(super) struct GraphqlCallContext", "private call context"],
  ["pub(super) fn new(query: &StorefrontPricingQuery)", "pre-call context constructor"],
  ["Uuid::new_v4()", "unique correlation id"],
  [
    '"pricing-storefront-graphql:{PRICING_STOREFRONT_GRAPHQL_OPERATION}:{}"',
    "correlation namespace",
  ],
  ["pub(super) fn map_error(&self, error: ApiError)", "final adapter error mapper"],
  ["let ApiError::Graphql(raw_error) = error else", "GraphQL-only remap"],
  ["return error;", "non-GraphQL pass-through"],
  ["let raw_error_present = !raw_error.trim().is_empty();", "raw display presence fact"],
  ["let raw_error_length = raw_error.chars().count();", "raw display length fact"],
  ["GraphqlHttpError::from_str(raw_error.as_str())", "known GraphQL HTTP classifier"],
  ["let parsed_error_valid = parsed_error.is_ok();", "typed parse validity fact"],
  ["Ok(GraphqlHttpError::Network)", "network classification"],
  ["Ok(GraphqlHttpError::Http(_))", "HTTP classification"],
  ["Ok(GraphqlHttpError::Unauthorized)", "authentication classification"],
  ["Ok(GraphqlHttpError::Graphql(_))", "GraphQL rejection classification"],
  ['"network"', "closed network category"],
  ['"http"', "closed HTTP category"],
  ['"unauthorized"', "closed authentication category"],
  ['"graphql"', "closed GraphQL category"],
  ['"unknown"', "closed unknown category"],
  ['"Storefront pricing is temporarily unavailable"', "unavailable public envelope"],
  ['"Pricing storefront authentication is required"', "authentication public envelope"],
  ['"Pricing storefront request could not be completed"', "request-rejected public envelope"],
  ['"pricing.storefront_graphql_network_unavailable"', "network stable code"],
  ['"pricing.storefront_graphql_http_unavailable"', "HTTP stable code"],
  ['"pricing.storefront_graphql_authentication_required"', "authentication stable code"],
  ['"pricing.storefront_graphql_request_rejected"', "request-rejected stable code"],
  ['"pricing.storefront_graphql_unknown_failure"', "unknown stable code"],
  ["raw_error_present,", "bounded raw display presence diagnostics"],
  ["raw_error_length,", "bounded raw display length diagnostics"],
  ["parsed_error_valid,", "bounded typed parse diagnostics"],
  ["correlation_id = %self.correlation_id", "correlation diagnostics"],
  ["tenant_slug_length = ?self.tenant_slug_length", "tenant shape diagnostics"],
  ["selected_handle_length = ?self.selected_handle_length", "handle shape diagnostics"],
  ["locale_length = ?self.locale_length", "locale shape diagnostics"],
  ["currency_code_length = ?self.currency_code_length", "currency shape diagnostics"],
  ["region_id_length = ?self.region_id_length", "region shape diagnostics"],
  ["price_list_id_length = ?self.price_list_id_length", "price-list shape diagnostics"],
  ["channel_id_length = ?self.channel_id_length", "channel-id shape diagnostics"],
  ["channel_slug_length = ?self.channel_slug_length", "channel-slug shape diagnostics"],
  ["quantity_present = self.quantity_present", "quantity presence diagnostics"],
  ["error_kind,", "closed error category diagnostics"],
  ["code,", "stable code diagnostics"],
  ["ApiError::Graphql(public_message.to_string())", "static public mapping"],
]) requireText(safety, marker, `${safetyPath}: ${label}`);

for (const marker of [
  "raw_error = %raw_error",
  "raw_error = ?raw_error",
  "parsed_error = ?parsed_error",
  "parsed_error = %parsed_error",
  "selected_handle = %",
  "selected_handle = ?",
  "locale = %",
  "locale = ?",
  "currency_code = %",
  "currency_code = ?",
  "region_id = %",
  "region_id = ?",
  "price_list_id = %",
  "price_list_id = ?",
  "channel_id = %",
  "channel_id = ?",
  "channel_slug = %",
  "channel_slug = ?",
  "quantity = %",
  "quantity = ?",
  "tenant_slug = %",
  "tenant_slug = ?",
  "query = ?query",
]) forbidText(safety, marker, `${safetyPath}: raw diagnostic or query payload`);

for (const marker of [
  "pub(crate) struct StorefrontPricingQuery",
  "pub(crate) selected_handle: Option<String>",
  "pub(crate) locale: Option<String>",
  "pub(crate) currency_code: Option<String>",
  "pub(crate) region_id: Option<String>",
  "pub(crate) price_list_id: Option<String>",
  "pub(crate) channel_id: Option<String>",
  "pub(crate) channel_slug: Option<String>",
  "pub(crate) quantity: Option<i32>",
]) requireText(core, marker, `${corePath}: query-shape source`);
for (const marker of [
  "pub enum GraphqlHttpError",
  "Network",
  "Graphql(String)",
  "Http(String)",
  "Unauthorized",
]) requireText(graphqlHttp, marker, `${graphqlHttpPath}: typed GraphQL HTTP contract`);
for (const marker of [
  "Storefront pricing is temporarily unavailable",
  "Pricing storefront context is unavailable",
  "fetch_storefront_pricing_server(query).await",
]) requireText(native, marker, `${nativePath}: preserved native public policy`);

if (evidence.schema_version !== 1) failures.push(`${evidencePath}: schema_version must be 1`);
if (evidence.status !== "pricing_storefront_graphql_error_safety_source_unvalidated") {
  failures.push(`${evidencePath}: status mismatch`);
}
for (const [key, expected] of Object.entries({
  context_before_graphql_call: true,
  unique_correlation_id: true,
  network_static_public_envelope: true,
  http_static_public_envelope: true,
  authentication_static_public_envelope: true,
  graphql_rejection_static_public_envelope: true,
  raw_graphql_http_display_public: false,
  raw_query_values_logged: false,
  raw_graphql_detail_logged: false,
  parsed_graphql_error_debug_logged: false,
  raw_graphql_detail_shape_logged: true,
  typed_parse_validity_logged: true,
  closed_graphql_error_category_logged: true,
  transport_validation_messages_preserved: true,
  native_adapter_changed: false,
  query_contract_changed: false,
  transport_selection_changed: false,
  fallback_added: false,
  broad_ecommerce_cleanup_closed: false,
})) {
  if (evidence.source_contract?.[key] !== expected) {
    failures.push(`${evidencePath}: source_contract.${key} must be ${expected}`);
  }
}
for (const key of [
  "tests_run",
  "cargo_run",
  "format_run",
  "focused_verifier_run",
  "aggregate_verifier_run",
  "broad_ecommerce_verifier_run",
  "workflow_checks_run",
  "ci_run",
  "graphql_runtime_proven",
  "browser_runtime_proven",
  "mounted_parity_proven",
]) {
  if (evidence.validation?.[key] !== false) {
    failures.push(`${evidencePath}: validation.${key} must remain false`);
  }
}
if (!Array.isArray(evidence.execution) || evidence.execution.length !== 0) {
  failures.push(`${evidencePath}: execution must remain empty`);
}
if (review.status !== "pricing_storefront_graphql_error_safety_source_reviewed_unvalidated") {
  failures.push(`${reviewPath}: status mismatch`);
}
for (const [key, expected] of Object.entries({
  typed_graphql_variant_policy: true,
  static_public_graphql_messages: true,
  raw_graphql_detail_logging_removed: true,
  parsed_graphql_error_debug_logging_removed: true,
  bounded_graphql_error_shape_retained: true,
  pricing_operation_preserved: true,
  per_call_correlation_id: true,
  safe_query_shape_only: true,
  private_graphql_adapter_changed: false,
  native_path_changed: false,
  query_contract_changed: false,
  transport_selection_changed: false,
  runtime_evidence_claimed: false,
})) {
  if (review.implementation_review?.[key] !== expected) {
    failures.push(`${reviewPath}: implementation_review.${key} must be ${expected}`);
  }
}
for (const marker of [
  "Status: **source-ready / unvalidated**",
  "Raw GraphQL display text is not written to the event.",
  "Debug output from the parsed typed error is not written to the event.",
  "raw-display presence and character length",
  "The master ecommerce correlation-safe mapper cleanup remains open",
  "No tests, verifiers, Cargo commands, formatting, workflows, or CI were run",
]) requireText(doc, marker, `${docPath}: truthful documentation`);
requireText(
  masterPlan,
  "Finish correlation-safe mapper cleanup",
  `${masterPlanPath}: broad mapper cleanup remains open`,
);

if (failures.length > 0) {
  console.error("Pricing storefront GraphQL error-safety verification failed:");
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  "✔ Pricing storefront GraphQL failures retain bounded error/query shape and static public envelopes; validation and runtime evidence remain unchanged",
);

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

const cargoPath = "crates/rustok-pricing/admin/Cargo.toml";
const transportPath = "crates/rustok-pricing/admin/src/transport.rs";
const graphqlPath = "crates/rustok-pricing/admin/src/transport/graphql_adapter.rs";
const safetyPath =
  "crates/rustok-pricing/admin/src/transport/graphql_error_safety.rs";
const nativePath =
  "crates/rustok-pricing/admin/src/transport/native_server_adapter.rs";
const evidencePath =
  "crates/rustok-pricing/contracts/evidence/admin-graphql-error-safety-source.json";
const reviewPath =
  "crates/rustok-pricing/contracts/evidence/admin-graphql-error-safety-source-review.json";
const docPath = "crates/rustok-pricing/docs/admin-graphql-error-safety.md";
const planPath = "crates/rustok-pricing/docs/implementation-plan.md";
const masterPlanPath = "crates/rustok-commerce/docs/implementation-plan.md";

const cargo = read(cargoPath);
const transport = read(transportPath);
const graphql = read(graphqlPath);
const safety = read(safetyPath);
const native = read(nativePath);
const evidence = JSON.parse(read(evidencePath));
const review = JSON.parse(read(reviewPath));
const doc = read(docPath);
const plan = read(planPath);
const masterPlan = read(masterPlanPath);

requireText(cargo, "tracing.workspace = true", `${cargoPath}: all-profile tracing`);
requireText(cargo, "uuid.workspace = true", `${cargoPath}: correlation UUID`);
requireText(
  transport,
  "mod graphql_error_safety;",
  `${transportPath}: private GraphQL policy module`,
);

for (const marker of [
  "GraphqlCallContext::for_bootstrap",
  "GraphqlCallContext::for_active_price_lists",
  "GraphqlCallContext::for_products",
  "GraphqlCallContext::for_product",
]) requireText(transport, marker, `${transportPath}: per-operation context`);
if (countText(transport, ".map_err(|error| context.map_error(error))") !== 4) {
  failures.push(`${transportPath}: exactly four GraphQL reads must use the final public mapper`);
}
for (const marker of [
  "graphql_adapter::fetch_bootstrap",
  "graphql_adapter::fetch_active_price_lists",
  "graphql_adapter::fetch_products",
  "graphql_adapter::fetch_product",
]) requireText(transport, marker, `${transportPath}: preserved GraphQL read`);
for (const marker of [
  "native_server_adapter::update_variant_price",
  "native_server_adapter::preview_variant_discount",
  "native_server_adapter::apply_variant_discount",
  "native_server_adapter::update_price_list_rule",
  "native_server_adapter::update_price_list_scope",
]) requireText(transport, marker, `${transportPath}: preserved native-only mutation`);

for (const marker of [
  "impl From<rustok_graphql::GraphqlHttpError> for ApiError",
  "Self::Graphql(value.to_string())",
  ".map_err(ApiError::from)",
  "BOOTSTRAP_QUERY",
  "ACTIVE_PRICE_LISTS_QUERY",
  "PRODUCTS_QUERY",
  "PRODUCT_QUERY",
]) requireText(graphql, marker, `${graphqlPath}: private GraphQL capture and contract`);

for (const marker of [
  "pub(super) struct GraphqlCallContext",
  "pricing-admin-graphql:{operation}:{}",
  "Uuid::new_v4()",
  "let ApiError::Graphql(raw_error) = error else",
  "return error;",
  "let raw_error_present = !raw_error.trim().is_empty();",
  "let raw_error_length = raw_error.chars().count();",
  "GraphqlHttpError::from_str(raw_error.as_str())",
  "let parsed_error_valid = parsed_error.is_ok();",
  "Ok(GraphqlHttpError::Network)",
  "Ok(GraphqlHttpError::Http(_))",
  "Ok(GraphqlHttpError::Unauthorized)",
  "Ok(GraphqlHttpError::Graphql(_))",
  '"network"',
  '"http"',
  '"unauthorized"',
  '"graphql"',
  '"unknown"',
  "Pricing admin service is temporarily unavailable",
  "Pricing admin authentication is required",
  "Pricing admin request could not be completed",
  "pricing.admin_graphql_network_unavailable",
  "pricing.admin_graphql_http_unavailable",
  "pricing.admin_graphql_authentication_required",
  "pricing.admin_graphql_request_rejected",
  "pricing.admin_graphql_unknown_failure",
  "raw_error_present,",
  "raw_error_length,",
  "parsed_error_valid,",
  "owner = PRICING_ADMIN_GRAPHQL_OWNER",
  "owner_operation = self.operation",
  "correlation_id = %self.correlation_id",
  "tenant_id_length = ?self.tenant_id_length",
  "resource_id_length = ?self.resource_id_length",
  "search_length = ?self.search_length",
  "quantity_present = self.quantity_present",
  "error_kind,",
  "code,",
  "ApiError::Graphql(public_message.to_string())",
]) requireText(safety, marker, `${safetyPath}: bounded GraphQL policy`);

for (const marker of [
  "raw_error = %raw_error",
  "raw_error = ?raw_error",
  "parsed_error = ?parsed_error",
  "parsed_error = %parsed_error",
  "tenant_slug = %",
  "tenant_slug = ?",
  "tenant_id = %",
  "tenant_id = ?",
  "resource_id = %",
  "resource_id = ?",
  "locale = %",
  "locale = ?",
  "search = %",
  "search = ?",
  "status = %",
  "status = ?",
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
]) forbidText(safety, marker, `${safetyPath}: raw diagnostic or request payload`);

requireText(native, "pub enum ApiError", `${nativePath}: shared error envelope`);
requireText(native, "pricing_admin_bootstrap_native", `${nativePath}: native read preserved`);
requireText(native, "pricing_admin_update_variant_price_native", `${nativePath}: native write preserved`);

if (evidence.schema_version !== 1) failures.push(`${evidencePath}: schema_version must be 1`);
if (evidence.status !== "pricing_admin_graphql_error_safety_source_unvalidated") {
  failures.push(`${evidencePath}: status mismatch`);
}
for (const [key, expected] of Object.entries({
  context_before_each_graphql_call: true,
  unique_correlation_id_per_call: true,
  network_static_public_envelope: true,
  http_static_public_envelope: true,
  authentication_static_public_envelope: true,
  graphql_rejection_static_public_envelope: true,
  raw_graphql_http_display_public: false,
  raw_request_values_logged: false,
  raw_graphql_detail_logged: false,
  parsed_graphql_error_debug_logged: false,
  raw_graphql_detail_shape_logged: true,
  typed_parse_validity_logged: true,
  closed_graphql_error_category_logged: true,
  request_validation_messages_preserved: true,
  native_adapter_changed: false,
  graphql_documents_changed: false,
  transport_selection_changed: false,
  fallback_added: false,
  native_only_mutations_changed: false,
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
if (review.status !== "pricing_admin_graphql_error_safety_source_reviewed_unvalidated") {
  failures.push(`${reviewPath}: status mismatch`);
}
for (const [key, expected] of Object.entries({
  typed_graphql_variant_policy: true,
  static_public_graphql_messages: true,
  raw_graphql_detail_logging_removed: true,
  parsed_graphql_error_debug_logging_removed: true,
  bounded_graphql_error_shape_retained: true,
  all_four_read_operations_preserved: true,
  per_call_correlation_id: true,
  safe_request_shape_only: true,
  private_graphql_adapter_changed: false,
  native_reads_changed: false,
  native_only_mutations_changed: false,
  graphql_documents_changed: false,
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
  "The ecommerce correlation-safe mapper cleanup remains open",
  "No tests, verifiers, Cargo commands, formatting, workflows, or CI were run",
]) requireText(doc, marker, `${docPath}: truthful documentation`);
requireText(plan, "admin GraphQL public errors", `${planPath}: local plan record`);
requireText(
  masterPlan,
  "Finish correlation-safe mapper cleanup",
  `${masterPlanPath}: broad mapper cleanup remains open`,
);

if (failures.length > 0) {
  console.error("Pricing admin GraphQL error-safety verification failed:");
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  "✔ Pricing admin GraphQL reads retain bounded error/request shape and static public envelopes; execution evidence remains maintainer-owned",
);

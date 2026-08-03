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
const between = (source, start, end, label) => {
  const from = source.indexOf(start);
  const to = source.indexOf(end, from + start.length);
  if (from < 0 || to < 0) {
    failures.push(`${label}: could not isolate ${start} before ${end}`);
    return "";
  }
  return source.slice(from, to);
};

const paths = {
  facade: "crates/rustok-product/admin/src/catalog_transport.rs",
  safety: "crates/rustok-product/admin/src/transport/graphql_error_safety.rs",
  legacy: "crates/rustok-product/admin/src/transport.rs",
  primaryEvidence:
    "crates/rustok-product/contracts/evidence/admin-primary-graphql-read-error-safety-source.json",
  primaryReview:
    "crates/rustok-product/contracts/evidence/admin-primary-graphql-read-error-safety-source-review.json",
  primaryDoc: "crates/rustok-product/docs/admin-primary-graphql-read-error-safety.md",
  categoryEvidence:
    "crates/rustok-product/contracts/evidence/admin-category-graphql-read-error-safety-source.json",
  categoryReview:
    "crates/rustok-product/contracts/evidence/admin-category-graphql-read-error-safety-source-review.json",
  categoryDoc: "crates/rustok-product/docs/admin-category-graphql-read-error-safety.md",
  masterPlan: "crates/rustok-commerce/docs/implementation-plan.md",
};

const facade = read(paths.facade);
const safety = read(paths.safety);
const legacy = read(paths.legacy);
const primaryEvidence = JSON.parse(read(paths.primaryEvidence));
const primaryReview = JSON.parse(read(paths.primaryReview));
const primaryDoc = read(paths.primaryDoc);
const categoryEvidence = JSON.parse(read(paths.categoryEvidence));
const categoryReview = JSON.parse(read(paths.categoryReview));
const categoryDoc = read(paths.categoryDoc);
const masterPlan = read(paths.masterPlan);

const readBlock = between(
  safety,
  "pub(super) struct GraphqlReadContext",
  "pub(super) struct GraphqlMutationContext",
  paths.safety,
);
const mutationBlock = safety.slice(safety.indexOf("pub(super) struct GraphqlMutationContext"));

for (const marker of [
  'const PRODUCT_ADMIN_GRAPHQL_BOUNDARY: &str = "product_admin_primary_graphql_reads";',
  '"product_admin_category_graphql_reads"',
  "pub(super) struct GraphqlReadContext",
  "pub(super) fn map_error(&self, error: GraphqlHttpError)",
  "GraphqlHttpError::Network",
  "GraphqlHttpError::Http(_)",
  "GraphqlHttpError::Unauthorized",
  "GraphqlHttpError::Graphql(_)",
  "let error_payload_length = match &error",
  "GraphqlHttpError::Http(value) | GraphqlHttpError::Graphql(value)",
  "Some(value.chars().count())",
  "GraphqlHttpError::Network | GraphqlHttpError::Unauthorized => None",
  "let error_payload_present = error_payload_length.is_some_and(|length| length > 0);",
  "error_payload_present,",
  "error_payload_length = ?error_payload_length",
  "tracing::error!(",
  "tracing::warn!(",
  "correlation_id = %self.correlation_id",
  "token_present = self.token_present",
  "tenant_slug_length = ?self.tenant_slug_length",
  "tenant_id_length = ?self.tenant_id_length",
  "resource_id_length = ?self.resource_id_length",
  "category_id_length = ?self.category_id_length",
  "locale_length = ?self.locale_length",
  "search_length = ?self.search_length",
  "status_length = ?self.status_length",
  "currency_code_length = ?self.currency_code_length",
  "native_fallback_attempted = self.native_fallback_attempted",
  "error_kind,",
  "code,",
  "boundary = self.boundary",
  '"Product admin service is temporarily unavailable"',
  '"Product admin request could not be completed"',
  '"product.admin_graphql_network_unavailable"',
  '"product.admin_graphql_http_unavailable"',
  '"product.admin_graphql_authentication_required"',
  '"product.admin_graphql_request_rejected"',
]) {
  requireText(readBlock, marker, `${paths.safety}: bounded read diagnostic contract`);
}

for (const marker of [
  "raw_error = ?error",
  "raw_error = %error",
  "error = ?error",
  "error = %error",
  "parsed_error = ?",
  "internal_message = %",
]) {
  forbidText(readBlock, marker, `${paths.safety}: complete read error payload`);
}

for (const marker of [
  "token = %",
  "token = ?",
  "tenant_slug = %",
  "tenant_slug = ?",
  "tenant_id = %",
  "tenant_id = ?",
  "resource_id = %",
  "resource_id = ?",
  "category_id = %",
  "category_id = ?",
  "locale = %",
  "locale = ?",
  "search = %",
  "search = ?",
  "status = %",
  "status = ?",
  "currency_code = %",
  "currency_code = ?",
]) {
  forbidText(readBlock, marker, `${paths.safety}: raw read request value`);
}

requireText(
  mutationBlock,
  "raw_error = ?error",
  `${paths.safety}: mutation diagnostic boundary remains explicitly open`,
);

const primaryContexts = [
  "GraphqlReadContext::for_bootstrap(",
  "GraphqlReadContext::for_products(",
  "GraphqlReadContext::for_product(",
  "GraphqlReadContext::for_product_pricing(",
  "GraphqlReadContext::for_shipping_profiles(",
];
const categoryContexts = [
  "GraphqlReadContext::for_product_attributes(",
  "GraphqlReadContext::for_catalog_categories(",
  "GraphqlReadContext::for_attribute_schemas(",
  "GraphqlReadContext::for_effective_product_form(",
  "GraphqlReadContext::for_product_attribute_values(",
];
for (const marker of [...primaryContexts, ...categoryContexts]) {
  requireText(facade, marker, `${paths.facade}: retained read context`);
}
requireText(
  facade,
  "admin_catalog_native::fetch_products(",
  `${paths.facade}: product-list native-first path`,
);
for (const marker of [
  "native_server_adapter::fetch_product_attributes(",
  "native_server_adapter::fetch_catalog_categories(",
  "native_server_adapter::fetch_attribute_schemas(",
  "native_server_adapter::fetch_effective_product_form(",
  "native_server_adapter::fetch_product_attribute_values(",
]) {
  requireText(legacy, marker, `${paths.legacy}: category native-first path`);
}

for (const [evidence, label] of [
  [primaryEvidence, paths.primaryEvidence],
  [categoryEvidence, paths.categoryEvidence],
]) {
  if (evidence.schema_version !== 1) failures.push(`${label}: schema_version must be 1`);
  for (const [key, expected] of Object.entries({
    typed_graphql_error_classification: true,
    raw_http_status_public: false,
    raw_graphql_message_public: false,
    complete_typed_error_logged: false,
    error_payload_shape_only: true,
    safe_request_shape_only: true,
    result_types_changed: false,
    graphql_documents_changed: false,
    graphql_variables_changed: false,
    response_mapping_changed: false,
    fallback_added: false,
  })) {
    if (evidence.source_contract?.[key] !== expected) {
      failures.push(`${label}: source_contract.${key} must be ${expected}`);
    }
  }
  if (evidence.safe_diagnostics?.includes("private_typed_graphql_error")) {
    failures.push(`${label}: safe_diagnostics must not retain private_typed_graphql_error`);
  }
  for (const marker of ["error_payload_present", "error_payload_length", "error_kind", "code"] ) {
    if (!evidence.safe_diagnostics?.includes(marker)) {
      failures.push(`${label}: safe_diagnostics must include ${marker}`);
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
    "browser_runtime_proven",
    "mounted_transport_proven",
  ]) {
    if (evidence.validation?.[key] !== false) {
      failures.push(`${label}: validation.${key} must remain false`);
    }
  }
  if (!Array.isArray(evidence.execution) || evidence.execution.length !== 0) {
    failures.push(`${label}: execution must remain empty`);
  }
}

if (
  primaryReview.status !==
  "product_admin_primary_graphql_read_error_safety_source_reviewed_unvalidated"
) {
  failures.push(`${paths.primaryReview}: status mismatch`);
}
if (
  categoryReview.status !==
  "product_admin_category_graphql_read_error_safety_source_reviewed_unvalidated"
) {
  failures.push(`${paths.categoryReview}: status mismatch`);
}
for (const [doc, label] of [
  [primaryDoc, paths.primaryDoc],
  [categoryDoc, paths.categoryDoc],
]) {
  requireText(doc, "Status: **source-ready / unvalidated**", `${label}: status`);
  requireText(doc, "payload presence and character length", `${label}: bounded payload policy`);
  requireText(doc, "complete typed error is not logged", `${label}: no full payload claim`);
  requireText(doc, "GraphQL writes and status mutations", `${label}: mutation boundary remains open`);
}
requireText(
  masterPlan,
  "Finish correlation-safe mapper cleanup",
  `${paths.masterPlan}: broad ecommerce cleanup remains open`,
);

if (failures.length > 0) {
  console.error("Product Admin GraphQL read diagnostic-safety verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  "Product Admin primary and category GraphQL reads retain typed classification with bounded payload-shape diagnostics; execution evidence remains open",
);

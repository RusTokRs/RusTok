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
const between = (source, start, end, label) => {
  const from = source.indexOf(start);
  const to = end ? source.indexOf(end, from + start.length) : source.length;
  if (from < 0 || to < 0) {
    failures.push(`${label}: could not isolate ${start}${end ? ` before ${end}` : ""}`);
    return "";
  }
  return source.slice(from, to);
};

const paths = {
  facade: "crates/rustok-product/admin/src/catalog_transport.rs",
  safety: "crates/rustok-product/admin/src/transport/graphql_error_safety.rs",
  legacy: "crates/rustok-product/admin/src/transport.rs",
  native: "crates/rustok-product/admin/src/transport/native_server_adapter.rs",
  graphql: "crates/rustok-product/admin/src/transport/graphql_adapter.rs",
  primaryGuard: "scripts/verify/verify-product-admin-primary-read-error-safety.mjs",
  catalogGuard: "scripts/verify/verify-product-admin-catalog-options-error-safety.mjs",
  evidence:
    "crates/rustok-product/contracts/evidence/admin-category-graphql-read-error-safety-source.json",
  review:
    "crates/rustok-product/contracts/evidence/admin-category-graphql-read-error-safety-source-review.json",
  doc: "crates/rustok-product/docs/admin-category-graphql-read-error-safety.md",
  masterPlan: "crates/rustok-commerce/docs/implementation-plan.md",
};

const facade = read(paths.facade);
const safety = read(paths.safety);
const legacy = read(paths.legacy);
const native = read(paths.native);
const graphql = read(paths.graphql);
const primaryGuard = read(paths.primaryGuard);
const catalogGuard = read(paths.catalogGuard);
const evidence = JSON.parse(read(paths.evidence));
const review = JSON.parse(read(paths.review));
const doc = read(paths.doc);
const masterPlan = read(paths.masterPlan);

const operations = [
  {
    name: "fetch_product_attributes",
    start: "pub(crate) async fn fetch_product_attributes(",
    end: "pub(crate) async fn fetch_catalog_categories(",
    context: "GraphqlReadContext::for_product_attributes(",
    facadeCall: "legacy::fetch_product_attributes(token, tenant_slug, tenant_id, locale)",
    nativeCall: "native_server_adapter::fetch_product_attributes(",
    graphqlCall: "graphql_adapter::fetch_product_attributes(",
  },
  {
    name: "fetch_catalog_categories",
    start: "pub(crate) async fn fetch_catalog_categories(",
    end: "pub(crate) async fn fetch_attribute_schemas(",
    context: "GraphqlReadContext::for_catalog_categories(",
    facadeCall: "legacy::fetch_catalog_categories(token, tenant_slug, tenant_id, locale)",
    nativeCall: "native_server_adapter::fetch_catalog_categories(",
    graphqlCall: "graphql_adapter::fetch_catalog_categories(",
  },
  {
    name: "fetch_attribute_schemas",
    start: "pub(crate) async fn fetch_attribute_schemas(",
    end: "pub(crate) async fn fetch_effective_product_form(",
    context: "GraphqlReadContext::for_attribute_schemas(",
    facadeCall: "legacy::fetch_attribute_schemas(token, tenant_slug, tenant_id, locale)",
    nativeCall: "native_server_adapter::fetch_attribute_schemas(",
    graphqlCall: "graphql_adapter::fetch_attribute_schemas(",
  },
  {
    name: "fetch_effective_product_form",
    start: "pub(crate) async fn fetch_effective_product_form(",
    end: "pub(crate) async fn fetch_product_attribute_values(",
    context: "GraphqlReadContext::for_effective_product_form(",
    facadeCall: "legacy::fetch_effective_product_form(",
    nativeCall: "native_server_adapter::fetch_effective_product_form(",
    graphqlCall: "graphql_adapter::fetch_effective_product_form(",
  },
  {
    name: "fetch_product_attribute_values",
    start: "pub(crate) async fn fetch_product_attribute_values(",
    end: null,
    context: "GraphqlReadContext::for_product_attribute_values(",
    facadeCall: "legacy::fetch_product_attribute_values(",
    nativeCall: "native_server_adapter::fetch_product_attribute_values(",
    graphqlCall: "graphql_adapter::fetch_product_attribute_values(",
  },
];

for (const operation of operations) {
  const facadeBlock = between(facade, operation.start, operation.end, paths.facade);
  for (const marker of [
    operation.context,
    operation.facadeCall,
    ".map_err(|failure| context.map_error(failure))",
  ]) {
    requireText(facadeBlock, marker, `${paths.facade}: ${operation.name} final boundary`);
  }
  const contextIndex = facadeBlock.indexOf(operation.context);
  const callIndex = facadeBlock.indexOf(operation.facadeCall);
  if (!(contextIndex >= 0 && callIndex >= 0 && contextIndex < callIndex)) {
    failures.push(`${paths.facade}: ${operation.name} context must precede the native-first executor`);
  }

  const legacyEnd =
    operation.name === "fetch_product_attributes"
      ? "pub(crate) async fn fetch_catalog_categories("
      : operation.name === "fetch_catalog_categories"
        ? "pub async fn fetch_catalog_search_options("
        : operation.name === "fetch_attribute_schemas"
          ? "pub(crate) async fn fetch_effective_product_form("
          : operation.name === "fetch_effective_product_form"
            ? "pub(crate) async fn fetch_product_attribute_values("
            : "pub(crate) async fn create_product(";
  const legacyBlock = between(legacy, operation.start, legacyEnd, paths.legacy);
  for (const marker of [operation.nativeCall, "Err(_) => {", operation.graphqlCall]) {
    requireText(legacyBlock, marker, `${paths.legacy}: ${operation.name} native-first fallback`);
  }
  const nativeIndex = legacyBlock.indexOf(operation.nativeCall);
  const graphqlIndex = legacyBlock.indexOf(operation.graphqlCall);
  if (!(nativeIndex >= 0 && graphqlIndex >= 0 && nativeIndex < graphqlIndex)) {
    failures.push(`${paths.legacy}: ${operation.name} native/GraphQL order drift`);
  }
}

if (countText(facade, ".map_err(|failure| context.map_error(failure))") !== 5) {
  failures.push(`${paths.facade}: expected exactly five category-read final mappers`);
}
if (countText(facade, ".map_err(|error| context.map_error(error))") !== 6) {
  failures.push(
    `${paths.facade}: prior five primary-read mappers and catalog-options mapper must remain intact`,
  );
}

for (const marker of [
  'const PRODUCT_ADMIN_CATEGORY_GRAPHQL_BOUNDARY: &str =',
  '"product_admin_category_graphql_reads"',
  "boundary: &'static str",
  "category_id_length: Option<usize>",
  "pub(super) fn for_product_attributes(",
  "pub(super) fn for_catalog_categories(",
  "pub(super) fn for_attribute_schemas(",
  "pub(super) fn for_effective_product_form(",
  "pub(super) fn for_product_attribute_values(",
  "fn for_category_tenant_locale(",
  "context.boundary = PRODUCT_ADMIN_CATEGORY_GRAPHQL_BOUNDARY;",
  "context.native_fallback_attempted = true;",
  "category_id_present = self.category_id_length.is_some()",
  "category_id_length = ?self.category_id_length",
  "boundary = self.boundary",
  "raw_error = ?error",
  '"Product admin service is temporarily unavailable"',
  '"Product admin request could not be completed"',
  '"product.admin_graphql_network_unavailable"',
  '"product.admin_graphql_http_unavailable"',
  '"product.admin_graphql_authentication_required"',
  '"product.admin_graphql_request_rejected"',
]) {
  requireText(safety, marker, `${paths.safety}: category read policy`);
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
]) {
  forbidText(safety, marker, `${paths.safety}: raw request values must not be logged`);
}

for (const marker of [
  "product_admin_attributes_native",
  "product_admin_categories_native",
  "product_admin_attribute_schemas_native",
  "product_admin_effective_form_native",
  "product_admin_attribute_values_native",
]) {
  requireText(native, marker, `${paths.native}: native owner read contract`);
}
for (const marker of [
  "PRODUCT_ATTRIBUTES_QUERY",
  "CATALOG_CATEGORIES_QUERY",
  "ATTRIBUTE_SCHEMAS_QUERY",
  "EFFECTIVE_FORM_QUERY",
  "ATTRIBUTE_VALUES_QUERY",
]) {
  requireText(graphql, marker, `${paths.graphql}: GraphQL category read contract`);
}
requireText(
  primaryGuard,
  "expected five primary read mappers plus the retained catalog-options mapper",
  `${paths.primaryGuard}: prior primary guard remains source-compatible`,
);
requireText(
  catalogGuard,
  "Product Admin catalog search-options error-safety verification failed:",
  `${paths.catalogGuard}: prior catalog-options guard remains present`,
);

if (evidence.schema_version !== 1) failures.push(`${paths.evidence}: schema_version must be 1`);
if (evidence.status !== "product_admin_category_graphql_read_error_safety_source_unvalidated") {
  failures.push(`${paths.evidence}: status mismatch`);
}
if (
  JSON.stringify(evidence.operations) !==
  JSON.stringify([
    "fetch_product_attributes",
    "fetch_catalog_categories",
    "fetch_attribute_schemas",
    "fetch_effective_product_form",
    "fetch_product_attribute_values",
  ])
) {
  failures.push(`${paths.evidence}: operation scope drift`);
}
for (const [key, expected] of Object.entries({
  final_public_wrapper: true,
  context_before_native_first_executor: true,
  native_first_preserved: true,
  graphql_fallback_preserved: true,
  unique_correlation_id: true,
  typed_graphql_error_classification: true,
  raw_http_status_public: false,
  raw_graphql_message_public: false,
  private_typed_error_diagnostics: true,
  safe_request_shape_only: true,
  result_types_changed: false,
  graphql_documents_changed: false,
  graphql_variables_changed: false,
  response_mapping_changed: false,
  retry_added: false,
  fallback_added: false,
})) {
  if (evidence.source_contract?.[key] !== expected) {
    failures.push(`${paths.evidence}: source_contract.${key} must be ${expected}`);
  }
}
for (const key of [
  "tests_run",
  "cargo_run",
  "format_run",
  "focused_verifier_run",
  "primary_read_verifier_run",
  "catalog_options_verifier_run",
  "aggregate_verifier_run",
  "broad_ecommerce_verifier_run",
  "workflow_checks_run",
  "ci_run",
  "browser_runtime_proven",
  "mounted_transport_proven",
]) {
  if (evidence.validation?.[key] !== false) {
    failures.push(`${paths.evidence}: validation.${key} must remain false`);
  }
}
if (!Array.isArray(evidence.execution) || evidence.execution.length !== 0) {
  failures.push(`${paths.evidence}: execution must remain empty`);
}
if (
  review.status !==
  "product_admin_category_graphql_read_error_safety_source_reviewed_unvalidated"
) {
  failures.push(`${paths.review}: status mismatch`);
}

requireText(doc, "Status: **source-ready / unvalidated**", `${paths.doc}: source status`);
requireText(doc, "five category-bound Product Admin reads", `${paths.doc}: operation scope`);
requireText(doc, "native-first executor", `${paths.doc}: preserved transport selection`);
requireText(doc, "Product admin service is temporarily unavailable", `${paths.doc}: HTTP policy`);
requireText(doc, "Product admin request could not be completed", `${paths.doc}: GraphQL policy`);
requireText(
  masterPlan,
  "Finish correlation-safe mapper cleanup",
  `${paths.masterPlan}: broad ecommerce mapper cleanup remains open`,
);

if (failures.length > 0) {
  console.error("Product Admin category GraphQL read error-safety verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  "Product Admin category GraphQL fallback reads retain native-first execution and correlation-safe static public errors; execution evidence remains open",
);

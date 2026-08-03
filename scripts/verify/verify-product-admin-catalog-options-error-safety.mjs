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

function requireText(source, value, label) {
  if (!source.includes(value)) failures.push(`${label}: missing ${value}`);
}

function forbidText(source, value, label) {
  if (source.includes(value)) failures.push(`${label}: forbidden ${value}`);
}

function countText(source, value) {
  return source.split(value).length - 1;
}

function between(source, start, end, label) {
  const from = source.indexOf(start);
  const to = source.indexOf(end, from + start.length);
  if (from < 0 || to < 0) {
    failures.push(`${label}: could not isolate ${start} before ${end}`);
    return "";
  }
  return source.slice(from, to);
}

const paths = {
  cargo: "crates/rustok-product/admin/Cargo.toml",
  lib: "crates/rustok-product/admin/src/lib.rs",
  publicTransport: "crates/rustok-product/admin/src/catalog_transport.rs",
  legacyTransport: "crates/rustok-product/admin/src/transport.rs",
  native: "crates/rustok-product/admin/src/transport/native_server_adapter.rs",
  graphql: "crates/rustok-product/admin/src/transport/graphql_adapter.rs",
  ui: "crates/rustok-product/admin/src/ui/catalog_admin.rs",
  evidence:
    "crates/rustok-product/contracts/evidence/admin-catalog-search-options-error-safety-source.json",
  review:
    "crates/rustok-product/contracts/evidence/admin-catalog-search-options-error-safety-source-review.json",
  doc: "crates/rustok-product/docs/admin-catalog-search-options-error-safety.md",
};

const cargo = read(paths.cargo);
const lib = read(paths.lib);
const publicTransport = read(paths.publicTransport);
const legacyTransport = read(paths.legacyTransport);
const native = read(paths.native);
const graphql = read(paths.graphql);
const ui = read(paths.ui);
const evidence = JSON.parse(read(paths.evidence));
const review = JSON.parse(read(paths.review));
const doc = read(paths.doc);

requireText(cargo, "uuid.workspace = true", `${paths.cargo}: correlation UUID dependency`);
requireText(cargo, "tracing.workspace = true", `${paths.cargo}: private diagnostics dependency`);

requireText(
  lib,
  "pub use transport::fetch_catalog_search_options;",
  `${paths.lib}: public Product Admin search-option contract`,
);
requireText(
  ui,
  "transport::fetch_catalog_search_options(token, tenant, locale).await",
  `${paths.ui}: UI must consume the public wrapper`,
);

forbidText(
  publicTransport,
  "pub use legacy::fetch_catalog_search_options;",
  `${paths.publicTransport}: raw legacy String must not be re-exported directly`,
);
for (const marker of [
  "pub async fn fetch_catalog_search_options(",
  "CatalogSearchOptionsErrorContext::new(",
  "legacy::fetch_catalog_search_options(token, tenant_slug, locale)",
  ".map_err(|error| context.map_error(error))",
]) {
  requireText(publicTransport, marker, `${paths.publicTransport}: final public error boundary`);
}

const wrapperStart = publicTransport.indexOf("pub async fn fetch_catalog_search_options(");
const productsStart = publicTransport.indexOf("pub(crate) async fn fetch_products(", wrapperStart);
if (wrapperStart < 0 || productsStart < 0) {
  failures.push(`${paths.publicTransport}: public search-options wrapper boundaries are missing`);
} else {
  const wrapper = publicTransport.slice(wrapperStart, productsStart);
  const contextIndex = wrapper.indexOf("CatalogSearchOptionsErrorContext::new(");
  const callIndex = wrapper.indexOf("legacy::fetch_catalog_search_options(");
  if (contextIndex < 0 || callIndex < 0 || contextIndex > callIndex) {
    failures.push(`${paths.publicTransport}: correlation context must be created before fallback execution`);
  }
}

for (const marker of [
  "struct CatalogSearchOptionsErrorContext",
  "uuid::Uuid::new_v4()",
  '"product-admin-catalog-options:{PRODUCT_ADMIN_CATALOG_OPTIONS_OPERATION}:{}"',
  "fn map_error(&self, raw_error: String) -> String",
  "let raw_error_present = !raw_error.is_empty();",
  "let raw_error_length = raw_error.chars().count();",
  "raw_error_present,",
  "raw_error_length,",
  "correlation_id = %self.correlation_id",
  "token_present = self.token_present",
  "tenant_slug_length = ?self.tenant_slug_length",
  "locale_length = self.locale_length",
  'code = "product.admin_catalog_search_options_graphql_unavailable"',
  '"Product catalog search options are temporarily unavailable"',
  "PRODUCT_ADMIN_CATALOG_OPTIONS_PUBLIC_MESSAGE.to_string()",
]) {
  requireText(publicTransport, marker, `${paths.publicTransport}: correlation-safe static mapping`);
}
for (const marker of [
  "raw_error = %raw_error",
  "raw_error = ?raw_error",
  "error = %raw_error",
  "error = ?raw_error",
  "token = %",
  "token = ?",
  "tenant_slug = %",
  "tenant_slug = ?",
  "locale = %",
  "locale = ?",
  "tenant_id = %",
  "tenant_id = ?",
]) {
  forbidText(publicTransport, marker, `${paths.publicTransport}: raw diagnostic or request values must not be logged`);
}

const legacyBlock = between(
  legacyTransport,
  "pub async fn fetch_catalog_search_options(",
  "fn first_non_empty",
  paths.legacyTransport,
);
for (const marker of [
  "native_server_adapter::fetch_catalog_search_options(locale.clone()).await",
  "graphql_adapter::fetch_bootstrap(token.clone(), tenant_slug.clone())",
  "graphql_adapter::fetch_catalog_categories(",
  "graphql_adapter::fetch_product_attributes(token, tenant_slug, tenant_id, locale)",
  "ProductCatalogSearchOptions {",
  "first_non_empty([category.path, category.name, category.code])",
  "attribute.is_filterable || attribute.is_sortable",
]) {
  requireText(legacyBlock, marker, `${paths.legacyTransport}: preserved fallback and projection contract`);
}
if (countText(legacyBlock, ".map_err(|err| err.to_string())?;") !== 3) {
  failures.push(
    `${paths.legacyTransport}: private compatibility executor must retain exactly three captured GraphQL String handoffs`,
  );
}
const nativeIndex = legacyBlock.indexOf("native_server_adapter::fetch_catalog_search_options");
const bootstrapIndex = legacyBlock.indexOf("graphql_adapter::fetch_bootstrap");
const categoriesIndex = legacyBlock.indexOf("graphql_adapter::fetch_catalog_categories");
const attributesIndex = legacyBlock.indexOf("graphql_adapter::fetch_product_attributes");
if (
  nativeIndex < 0 ||
  bootstrapIndex < 0 ||
  categoriesIndex < 0 ||
  attributesIndex < 0 ||
  !(nativeIndex < bootstrapIndex && bootstrapIndex < categoriesIndex && categoriesIndex < attributesIndex)
) {
  failures.push(`${paths.legacyTransport}: native/bootstrap/categories/attributes order drift`);
}

for (const marker of [
  "pub(super) async fn fetch_catalog_search_options(",
  "product_admin_catalog_search_options_native(locale)",
]) {
  requireText(native, marker, `${paths.native}: native-first owner endpoint`);
}
for (const marker of [
  "pub type ApiError = GraphqlHttpError;",
  "ProductAdminBootstrap",
  "ProductAdminCatalogCategories",
  "ProductAdminAttributes",
]) {
  requireText(graphql, marker, `${paths.graphql}: preserved GraphQL owner contracts`);
}

if (evidence.schema_version !== 1) failures.push(`${paths.evidence}: schema_version must be 1`);
if (
  evidence.status !==
  "product_admin_catalog_search_options_error_safety_source_unvalidated"
) {
  failures.push(`${paths.evidence}: status mismatch`);
}
for (const [key, expected] of Object.entries({
  native_first_preserved: true,
  graphql_bootstrap_categories_attributes_order_preserved: true,
  legacy_raw_error_capture_private: false,
  raw_error_shape_only: true,
  raw_graphql_error_logged: false,
  raw_graphql_error_public: false,
  unique_correlation_id: true,
  safe_request_shape_only: true,
  token_value_logged: false,
  tenant_slug_value_logged: false,
  locale_value_logged: false,
  public_result_type_changed: false,
  fallback_added: false,
  transport_selection_changed: false,
})) {
  if (evidence.source_contract?.[key] !== expected) {
    failures.push(`${paths.evidence}: source_contract.${key} must be ${expected}`);
  }
}
for (const marker of [
  "raw_error_present",
  "raw_error_length",
]) {
  if (!evidence.safe_diagnostics?.includes(marker)) {
    failures.push(`${paths.evidence}: safe_diagnostics must include ${marker}`);
  }
}
if (evidence.safe_diagnostics?.includes("private_raw_graphql_error")) {
  failures.push(`${paths.evidence}: safe_diagnostics must not retain private_raw_graphql_error`);
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
  "mounted_fallback_proven",
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
  "product_admin_catalog_search_options_error_safety_source_reviewed_unvalidated"
) {
  failures.push(`${paths.review}: status mismatch`);
}
requireText(doc, "Status: **source-ready / unvalidated**", `${paths.doc}: source status`);
requireText(
  doc,
  "Product catalog search options are temporarily unavailable",
  `${paths.doc}: static public contract`,
);
requireText(doc, "native-first fallback policy", `${paths.doc}: preserved fallback policy`);
requireText(doc, "does not write the captured error text", `${paths.doc}: bounded diagnostic policy`);

if (failures.length > 0) {
  console.error("Product Admin catalog search-options error-safety verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  "Product Admin catalog search-option failures use one static public String and bounded private diagnostics; execution evidence remains open",
);

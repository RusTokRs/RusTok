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
  const to = end ? source.indexOf(end, from + start.length) : source.length;
  if (from < 0 || to < 0) {
    failures.push(`${label}: could not isolate ${start}${end ? ` before ${end}` : ""}`);
    return "";
  }
  return source.slice(from, to);
};

const paths = {
  facade: "crates/modules/rustok-product/admin/src/catalog_transport.rs",
  legacy: "crates/modules/rustok-product/admin/src/transport.rs",
  native: "crates/modules/rustok-product/admin/src/transport/native_server_adapter.rs",
  graphql: "crates/modules/rustok-product/admin/src/transport/graphql_adapter.rs",
  lib: "crates/modules/rustok-product/admin/src/lib.rs",
  catalogUi: "crates/modules/rustok-product/admin/src/ui/catalog_admin.rs",
  gridUi: "crates/modules/rustok-product/admin/src/ui/product_grid.rs",
  evidence: "crates/modules/rustok-product/contracts/evidence/admin-catalog-search-options-error-safety-source.json",
  review: "crates/modules/rustok-product/contracts/evidence/admin-catalog-search-options-error-safety-source-review.json",
  doc: "crates/modules/rustok-product/docs/admin-catalog-search-options-error-safety.md",
};

const facade = read(paths.facade);
const legacy = read(paths.legacy);
const native = read(paths.native);
const graphql = read(paths.graphql);
const lib = read(paths.lib);
const catalogUi = read(paths.catalogUi);
const gridUi = read(paths.gridUi);
const evidence = JSON.parse(read(paths.evidence));
const review = JSON.parse(read(paths.review));
const doc = read(paths.doc);

const wrapper = between(
  facade,
  "pub async fn fetch_catalog_search_options(",
  "pub(crate) async fn fetch_products(",
  paths.facade,
);
const legacyOptions = between(
  legacy,
  "pub(crate) async fn fetch_catalog_search_options(",
  "#[cfg(any(not(any(feature = \"ssr\", feature = \"hydrate\")), test))]",
  paths.legacy,
);
const nativeBranch = between(
  legacyOptions,
  '#[cfg(any(feature = "ssr", feature = "hydrate"))]',
  '#[cfg(not(any(feature = "ssr", feature = "hydrate")))]',
  paths.legacy,
);
const graphqlBranch = legacyOptions.slice(
  legacyOptions.indexOf('#[cfg(not(any(feature = "ssr", feature = "hydrate")))]'),
);
const structuralOptions = between(
  legacy,
  "fn structural_category_options(",
  "fn filterable_attribute_options(",
  paths.legacy,
);
const attributeOptions = between(
  legacy,
  "fn filterable_attribute_options(",
  "pub(crate) async fn fetch_catalog_search_options(",
  paths.legacy,
);
const errorContext = between(
  facade,
  "struct CatalogSearchOptionsErrorContext",
  "pub(crate) async fn fetch_products(",
  paths.facade,
);
const nativeContext = between(
  native,
  "pub(super) async fn ensure_product_module_enabled(",
  "async fn native_context()",
  paths.native,
);

for (const marker of [
  "CatalogSearchOptionsErrorContext::new(",
  "legacy::fetch_catalog_search_options(token, tenant_slug, locale)",
  ".map_err(|error| context.map_error(error))",
]) {
  requireText(wrapper, marker, `${paths.facade}: public search-options boundary`);
}
for (const marker of [
  "correlation_id_present = !self.correlation_id.is_empty()",
  "correlation_id_length = self.correlation_id.chars().count()",
  "selected_transport = self.selected_transport",
  "raw_error_present",
  "raw_error_length",
  "token_present",
  "tenant_slug_length",
  "locale_length",
  "product.admin_catalog_search_options_unavailable",
  "PRODUCT_ADMIN_CATALOG_OPTIONS_PUBLIC_MESSAGE.to_string()",
]) {
  requireText(errorContext, marker, `${paths.facade}: bounded selected-transport diagnostics`);
}
for (const marker of [
  "correlation_id = %self.correlation_id",
  "raw_error = %raw_error",
  "raw_error = ?raw_error",
  "tenant_slug = %",
  "locale = %",
  "token = %",
]) {
  forbidText(errorContext, marker, `${paths.facade}: raw search-options diagnostics`);
}

for (const marker of [
  '#[cfg(any(feature = "ssr", feature = "hydrate"))]',
  "native_server_adapter::fetch_catalog_search_options(locale)",
  ".map_err(|error| error.to_string())",
]) {
  requireText(nativeBranch, marker, `${paths.legacy}: native SSR/hydrate selection`);
}
forbidText(nativeBranch, "graphql_adapter::", `${paths.legacy}: native failure must not retry over GraphQL`);
for (const marker of [
  '#[cfg(not(any(feature = "ssr", feature = "hydrate")))]',
  "graphql_adapter::fetch_bootstrap(token.clone(), tenant_slug.clone())",
  "graphql_adapter::fetch_catalog_categories(",
  "graphql_adapter::fetch_product_attributes(",
  "category_options: structural_category_options(categories.items)",
  "attribute_options: filterable_attribute_options(attributes.items)",
]) {
  requireText(graphqlBranch, marker, `${paths.legacy}: GraphQL CSR selection and DTO projection`);
}
forbidText(graphqlBranch, "native_server_adapter::", `${paths.legacy}: CSR failure must not retry over native`);
forbidText(legacyOptions, "match native_server_adapter::", `${paths.legacy}: no native-first fallback`);
forbidText(legacyOptions, "Err(_) =>", `${paths.legacy}: no cross-transport retry`);

requireText(
  structuralOptions,
  '.filter(|category| category.kind.eq_ignore_ascii_case("structural"))',
  `${paths.legacy}: category filters must exclude non-structural categories`,
);
requireText(
  structuralOptions,
  "first_non_empty([category.path, category.name, category.code])",
  `${paths.legacy}: category label precedence`,
);
requireText(
  attributeOptions,
  'attribute.is_filterable && matches!(attribute.scope.as_str(), "product" | "both")',
  `${paths.legacy}: attribute filters must be filterable Product/Both attributes`,
);

requireText(native, "product_admin_catalog_search_options_native(locale)", `${paths.native}: native options endpoint`);
requireText(native, "ensure_product_module_enabled(&runtime_ctx, tenant.id).await?", `${paths.native}: native tenant module gate`);
requireText(native, 'is_tenant_module_enabled(runtime.db(), tenant_id, "product")', `${paths.native}: Product module gate implementation`);
requireText(graphql, "PRODUCT_ATTRIBUTES_QUERY", `${paths.graphql}: GraphQL attribute query`);
requireText(graphql, "scope", `${paths.graphql}: GraphQL attribute scope projection`);
requireText(graphql, "isFilterable", `${paths.graphql}: GraphQL filterable attribute projection`);
requireText(lib, "pub use catalog_transport::fetch_catalog_search_options;", `${paths.lib}: public export must use the safe facade`);
forbidText(lib, "pub use transport::fetch_catalog_search_options;", `${paths.lib}: raw transport export bypasses the public error boundary`);
requireText(legacy, "pub(crate) async fn fetch_catalog_search_options(", `${paths.legacy}: raw executor must remain crate-private`);
requireText(catalogUi, "catalog_transport::fetch_catalog_search_options(token, tenant, locale).await", `${paths.catalogUi}: reference UI uses the safe facade`);
requireText(gridUi, "catalog_transport::fetch_catalog_search_options(", `${paths.gridUi}: mounted grid uses the safe facade`);

if (evidence.schema_version !== 1) failures.push(`${paths.evidence}: schema_version must be 1`);
if (evidence.operation !== "fetch_catalog_search_options") failures.push(`${paths.evidence}: operation drift`);
if (evidence.source_contract?.native_selected_in_ssr_hydrate !== true) failures.push(`${paths.evidence}: native_selected_in_ssr_hydrate must be true`);
if (evidence.source_contract?.graphql_selected_in_csr !== true) failures.push(`${paths.evidence}: graphql_selected_in_csr must be true`);
if (evidence.source_contract?.cross_transport_retry !== false) failures.push(`${paths.evidence}: cross_transport_retry must be false`);
if (evidence.source_contract?.transport_selection_changed !== true) failures.push(`${paths.evidence}: transport_selection_changed must be true`);
if (evidence.source_contract?.raw_error_shape_only !== true) failures.push(`${paths.evidence}: raw_error_shape_only must be true`);
if (evidence.source_contract?.raw_graphql_error_logged !== false) failures.push(`${paths.evidence}: raw_graphql_error_logged must be false`);
if (evidence.source_contract?.raw_graphql_error_public !== false) failures.push(`${paths.evidence}: raw_graphql_error_public must be false`);
if (evidence.validation?.tests_run !== false || evidence.validation?.cargo_run !== false || evidence.validation?.format_run !== false) {
  failures.push(`${paths.evidence}: validation must remain unexecuted`);
}
if (!Array.isArray(evidence.execution) || evidence.execution.length !== 0) failures.push(`${paths.evidence}: execution must remain empty`);
if (review.status !== "product_admin_catalog_search_options_error_safety_source_reviewed_unvalidated") {
  failures.push(`${paths.review}: review status mismatch`);
}
if (review.review_base !== "f94c88fc06aa7e6285d4a5cf82fa3e531c3fe25a") {
  failures.push(`${paths.review}: review_base must match the current main baseline`);
}
for (const marker of [
  "Status: **source-ready / unvalidated**",
  "SSR/hydrate",
  "CSR",
  "Failure ends the operation in the selected transport.",
  "Product catalog search options are temporarily unavailable",
  "structural",
  "Product or Both",
  "ProductCatalogSearchOptions",
]) {
  requireText(doc, marker, `${paths.doc}: selected transport contract`);
}

if (failures.length > 0) {
  console.error("Product Admin catalog search-options error-safety verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  "Product Admin catalog search options use one build-profile-selected transport, preserve bounded diagnostics, and expose only the safe public facade; execution evidence remains open",
);

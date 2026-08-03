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
  cargo: "crates/rustok-product/admin/Cargo.toml",
  facade: "crates/rustok-product/admin/src/catalog_transport.rs",
  safety: "crates/rustok-product/admin/src/transport/graphql_error_safety.rs",
  legacy: "crates/rustok-product/admin/src/transport.rs",
  listGraphql: "crates/rustok-product/admin/src/transport/admin_catalog_graphql.rs",
  native: "crates/rustok-product/admin/src/transport/native_server_adapter.rs",
  graphql: "crates/rustok-product/admin/src/transport/graphql_adapter.rs",
  graphqlHttp: "crates/rustok-graphql/src/lib.rs",
  ui: "crates/rustok-product/admin/src/ui/leptos.rs",
  catalogGuard: "scripts/verify/verify-product-admin-catalog-options-error-safety.mjs",
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

const cargo = read(paths.cargo);
const facade = read(paths.facade);
const safety = read(paths.safety);
const legacy = read(paths.legacy);
const listGraphql = read(paths.listGraphql);
const native = read(paths.native);
const graphql = read(paths.graphql);
const graphqlHttp = read(paths.graphqlHttp);
const ui = read(paths.ui);
const catalogGuard = read(paths.catalogGuard);
const primaryEvidence = JSON.parse(read(paths.primaryEvidence));
const primaryReview = JSON.parse(read(paths.primaryReview));
const primaryDoc = read(paths.primaryDoc);
const categoryEvidence = JSON.parse(read(paths.categoryEvidence));
const categoryReview = JSON.parse(read(paths.categoryReview));
const categoryDoc = read(paths.categoryDoc);
const masterPlan = read(paths.masterPlan);

requireText(cargo, "uuid.workspace = true", `${paths.cargo}: correlation UUID dependency`);
requireText(cargo, "tracing.workspace = true", `${paths.cargo}: diagnostics dependency`);
for (const marker of [
  '#[path = "transport/graphql_error_safety.rs"]',
  "mod graphql_error_safety;",
  "use rustok_graphql::GraphqlHttpError;",
]) {
  requireText(facade, marker, `${paths.facade}: typed policy wiring`);
}

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
  "boundary: &'static str",
  "category_id_length: Option<usize>",
  "Uuid::new_v4()",
  '"product-admin-graphql:{operation}:{}"',
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
  "public_error",
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

const primaryOperations = [
  {
    name: "fetch_bootstrap",
    start: "pub(crate) async fn fetch_bootstrap(",
    end: "pub async fn fetch_catalog_search_options(",
    context: "GraphqlReadContext::for_bootstrap(",
    call: "legacy::fetch_bootstrap(token, tenant_slug)",
  },
  {
    name: "fetch_products",
    start: "pub(crate) async fn fetch_products(",
    end: "pub(crate) async fn fetch_product(",
    context: "GraphqlReadContext::for_products(",
    call: "admin_catalog_graphql::fetch_products(",
  },
  {
    name: "fetch_product",
    start: "pub(crate) async fn fetch_product(",
    end: "pub(crate) async fn fetch_product_pricing(",
    context: "GraphqlReadContext::for_product(",
    call: "legacy::fetch_product(token, tenant_slug, tenant_id, id, locale)",
  },
  {
    name: "fetch_product_pricing",
    start: "pub(crate) async fn fetch_product_pricing(",
    end: "pub(crate) async fn fetch_shipping_profiles(",
    context: "GraphqlReadContext::for_product_pricing(",
    call: "legacy::fetch_product_pricing(",
  },
];
for (const operation of primaryOperations) {
  const block = between(facade, operation.start, operation.end, paths.facade);
  for (const marker of [operation.context, operation.call, ".map_err(|error| context.map_error(error))"]) {
    requireText(block, marker, `${paths.facade}: ${operation.name} final read boundary`);
  }
  const contextIndex = block.indexOf(operation.context);
  const callIndex = block.indexOf(operation.call);
  if (!(contextIndex >= 0 && callIndex >= 0 && contextIndex < callIndex)) {
    failures.push(`${paths.facade}: ${operation.name} context must precede GraphQL execution`);
  }
}
const shippingStart = facade.indexOf("pub(crate) async fn fetch_shipping_profiles(");
const shipping = shippingStart < 0 ? "" : facade.slice(shippingStart);
for (const marker of [
  "GraphqlReadContext::for_shipping_profiles(",
  "legacy::fetch_shipping_profiles(token, tenant_slug, tenant_id)",
  ".map_err(|error| context.map_error(error))",
]) {
  requireText(shipping, marker, `${paths.facade}: shipping final read boundary`);
}
if (
  shipping.indexOf("GraphqlReadContext::for_shipping_profiles(") >
  shipping.indexOf("legacy::fetch_shipping_profiles(")
) {
  failures.push(`${paths.facade}: shipping context must precede GraphQL execution`);
}
if (countText(facade, ".map_err(|error| context.map_error(error))") !== 6) {
  failures.push(`${paths.facade}: expected five primary read mappers plus catalog-options mapper`);
}

const productsBlock = between(
  facade,
  "pub(crate) async fn fetch_products(",
  "pub(crate) async fn fetch_product(",
  paths.facade,
);
for (const marker of [
  "admin_catalog_native::fetch_products(",
  "Err(_) => {",
  "GraphqlReadContext::for_products(",
  "admin_catalog_graphql::fetch_products(",
]) {
  requireText(productsBlock, marker, `${paths.facade}: product-list native-first contract`);
}
const productsNativeIndex = productsBlock.indexOf("admin_catalog_native::fetch_products(");
const productsContextIndex = productsBlock.indexOf("GraphqlReadContext::for_products(");
const productsGraphqlIndex = productsBlock.indexOf("admin_catalog_graphql::fetch_products(");
if (!(productsNativeIndex >= 0 && productsNativeIndex < productsContextIndex && productsContextIndex < productsGraphqlIndex)) {
  failures.push(`${paths.facade}: product-list native/context/GraphQL order drift`);
}

const categoryOperations = [
  {
    name: "fetch_product_attributes",
    start: "pub(crate) async fn fetch_product_attributes(",
    facadeEnd: "pub(crate) async fn fetch_catalog_categories(",
    legacyEnd: "pub(crate) async fn fetch_catalog_categories(",
    context: "GraphqlReadContext::for_product_attributes(",
    facadeCall: "legacy::fetch_product_attributes(token, tenant_slug, tenant_id, locale)",
    nativeCall: "native_server_adapter::fetch_product_attributes(",
    graphqlCall: "graphql_adapter::fetch_product_attributes(",
  },
  {
    name: "fetch_catalog_categories",
    start: "pub(crate) async fn fetch_catalog_categories(",
    facadeEnd: "pub(crate) async fn fetch_attribute_schemas(",
    legacyEnd: "pub async fn fetch_catalog_search_options(",
    context: "GraphqlReadContext::for_catalog_categories(",
    facadeCall: "legacy::fetch_catalog_categories(token, tenant_slug, tenant_id, locale)",
    nativeCall: "native_server_adapter::fetch_catalog_categories(",
    graphqlCall: "graphql_adapter::fetch_catalog_categories(",
  },
  {
    name: "fetch_attribute_schemas",
    start: "pub(crate) async fn fetch_attribute_schemas(",
    facadeEnd: "pub(crate) async fn fetch_effective_product_form(",
    legacyEnd: "pub(crate) async fn fetch_effective_product_form(",
    context: "GraphqlReadContext::for_attribute_schemas(",
    facadeCall: "legacy::fetch_attribute_schemas(token, tenant_slug, tenant_id, locale)",
    nativeCall: "native_server_adapter::fetch_attribute_schemas(",
    graphqlCall: "graphql_adapter::fetch_attribute_schemas(",
  },
  {
    name: "fetch_effective_product_form",
    start: "pub(crate) async fn fetch_effective_product_form(",
    facadeEnd: "pub(crate) async fn fetch_product_attribute_values(",
    legacyEnd: "pub(crate) async fn fetch_product_attribute_values(",
    context: "GraphqlReadContext::for_effective_product_form(",
    facadeCall: "legacy::fetch_effective_product_form(",
    nativeCall: "native_server_adapter::fetch_effective_product_form(",
    graphqlCall: "graphql_adapter::fetch_effective_product_form(",
  },
  {
    name: "fetch_product_attribute_values",
    start: "pub(crate) async fn fetch_product_attribute_values(",
    facadeEnd: "pub(crate) async fn create_product(",
    legacyEnd: "pub(crate) async fn create_product(",
    context: "GraphqlReadContext::for_product_attribute_values(",
    facadeCall: "legacy::fetch_product_attribute_values(",
    nativeCall: "native_server_adapter::fetch_product_attribute_values(",
    graphqlCall: "graphql_adapter::fetch_product_attribute_values(",
  },
];
for (const operation of categoryOperations) {
  const facadeBlock = between(facade, operation.start, operation.facadeEnd, paths.facade);
  for (const marker of [
    operation.context,
    operation.facadeCall,
    ".map_err(|failure| context.map_error(failure))",
  ]) {
    requireText(facadeBlock, marker, `${paths.facade}: ${operation.name} final read boundary`);
  }
  const contextIndex = facadeBlock.indexOf(operation.context);
  const callIndex = facadeBlock.indexOf(operation.facadeCall);
  if (!(contextIndex >= 0 && callIndex >= 0 && contextIndex < callIndex)) {
    failures.push(`${paths.facade}: ${operation.name} context must precede native-first executor`);
  }

  const legacyBlock = between(legacy, operation.start, operation.legacyEnd, paths.legacy);
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

for (const marker of [
  "graphql_adapter::fetch_bootstrap(token, tenant_slug).await",
  "graphql_adapter::fetch_product(token, tenant_slug, tenant_id, id, locale).await",
  "graphql_adapter::fetch_product_pricing(",
  "graphql_adapter::fetch_shipping_profiles(token, tenant_slug, tenant_id).await",
]) {
  requireText(legacy, marker, `${paths.legacy}: preserved primary GraphQL delegation`);
}
for (const marker of ["ADMIN_PRODUCT_CATALOG_QUERY", "page: Some(1)", "per_page: Some(24)"]) {
  requireText(listGraphql, marker, `${paths.listGraphql}: product-list query contract`);
}
for (const marker of [
  "pub type ApiError = GraphqlHttpError;",
  "BOOTSTRAP_QUERY",
  "PRODUCT_QUERY",
  "PRODUCT_PRICING_QUERY",
  "SHIPPING_PROFILES_QUERY",
  "PRODUCT_ATTRIBUTES_QUERY",
  "CATALOG_CATEGORIES_QUERY",
  "ATTRIBUTE_SCHEMAS_QUERY",
  "EFFECTIVE_FORM_QUERY",
  "ATTRIBUTE_VALUES_QUERY",
]) {
  requireText(graphql, marker, `${paths.graphql}: retained GraphQL read contract`);
}
for (const marker of [
  "product_admin_attributes_native",
  "product_admin_categories_native",
  "product_admin_attribute_schemas_native",
  "product_admin_effective_form_native",
  "product_admin_attribute_values_native",
]) {
  requireText(native, marker, `${paths.native}: retained native read contract`);
}
for (const marker of [
  "pub enum GraphqlHttpError",
  "Graphql(String)",
  "Http(String)",
  "Unauthorized",
]) {
  requireText(graphqlHttp, marker, `${paths.graphqlHttp}: typed HTTP error contract`);
}
for (const marker of [
  "transport::fetch_bootstrap(",
  "transport::fetch_products(",
  "transport::fetch_product(",
  "transport::fetch_product_pricing(",
  "transport::fetch_shipping_profiles(",
  "transport::fetch_product_attributes(",
  "transport::fetch_catalog_categories(",
  "transport::fetch_attribute_schemas(",
  "transport::fetch_effective_product_form(",
  "transport::fetch_product_attribute_values(",
]) {
  requireText(ui, marker, `${paths.ui}: retained UI resource composition`);
}
requireText(
  catalogGuard,
  "Product Admin catalog search-options error-safety verification failed:",
  `${paths.catalogGuard}: prior catalog-options guard remains present`,
);

const expectedPrimaryOperations = [
  "fetch_bootstrap",
  "fetch_products",
  "fetch_product",
  "fetch_product_pricing",
  "fetch_shipping_profiles",
];
const expectedCategoryOperations = [
  "fetch_product_attributes",
  "fetch_catalog_categories",
  "fetch_attribute_schemas",
  "fetch_effective_product_form",
  "fetch_product_attribute_values",
];
if (primaryEvidence.schema_version !== 1) {
  failures.push(`${paths.primaryEvidence}: schema_version must be 1`);
}
if (categoryEvidence.schema_version !== 1) {
  failures.push(`${paths.categoryEvidence}: schema_version must be 1`);
}
if (JSON.stringify(primaryEvidence.operations) !== JSON.stringify(expectedPrimaryOperations)) {
  failures.push(`${paths.primaryEvidence}: operation scope drift`);
}
if (JSON.stringify(categoryEvidence.operations) !== JSON.stringify(expectedCategoryOperations)) {
  failures.push(`${paths.categoryEvidence}: operation scope drift`);
}
for (const [evidence, label, extra] of [
  [
    primaryEvidence,
    paths.primaryEvidence,
    {
      context_before_graphql_call: true,
      product_list_native_first_preserved: true,
      product_list_graphql_fallback_preserved: true,
    },
  ],
  [
    categoryEvidence,
    paths.categoryEvidence,
    {
      final_public_wrapper: true,
      context_before_native_first_executor: true,
      native_first_preserved: true,
      graphql_fallback_preserved: true,
      retry_added: false,
    },
  ],
]) {
  for (const [key, expected] of Object.entries({
    unique_correlation_id: true,
    typed_graphql_error_classification: true,
    network_static_public_envelope: true,
    http_static_public_envelope: true,
    unauthorized_static_public_envelope: true,
    graphql_static_public_envelope: true,
    raw_http_status_public: false,
    raw_graphql_message_public: false,
    complete_typed_error_logged: false,
    error_payload_shape_only: true,
    private_typed_error_classification: true,
    safe_request_shape_only: true,
    result_types_changed: false,
    graphql_documents_changed: false,
    graphql_variables_changed: false,
    response_mapping_changed: false,
    fallback_added: false,
    ...extra,
  })) {
    if (evidence.source_contract?.[key] !== expected) {
      failures.push(`${label}: source_contract.${key} must be ${expected}`);
    }
  }
  if (evidence.safe_diagnostics?.includes("private_typed_graphql_error")) {
    failures.push(`${label}: safe_diagnostics must not retain private_typed_graphql_error`);
  }
  for (const marker of [
    "error_payload_present",
    "error_payload_length",
    "error_kind",
    "code",
    "boundary",
  ]) {
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
  requireText(doc, "Product admin service is temporarily unavailable", `${label}: HTTP policy`);
  requireText(doc, "Product admin request could not be completed", `${label}: GraphQL policy`);
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
  "Product Admin primary and category GraphQL reads retain complete transport and UI contracts with bounded payload-shape diagnostics; execution evidence remains open",
);

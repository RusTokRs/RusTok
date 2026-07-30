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
  const to = source.indexOf(end, from + start.length);
  if (from < 0 || to < 0) {
    failures.push(`${label}: could not isolate ${start} before ${end}`);
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
  graphql: "crates/rustok-product/admin/src/transport/graphql_adapter.rs",
  graphqlHttp: "crates/rustok-graphql/src/lib.rs",
  ui: "crates/rustok-product/admin/src/ui/leptos.rs",
  priorGuard: "scripts/verify/verify-product-admin-catalog-options-error-safety.mjs",
  evidence:
    "crates/rustok-product/contracts/evidence/admin-primary-graphql-read-error-safety-source.json",
  review:
    "crates/rustok-product/contracts/evidence/admin-primary-graphql-read-error-safety-source-review.json",
  doc: "crates/rustok-product/docs/admin-primary-graphql-read-error-safety.md",
  masterPlan: "crates/rustok-commerce/docs/implementation-plan.md",
};

const cargo = read(paths.cargo);
const facade = read(paths.facade);
const safety = read(paths.safety);
const legacy = read(paths.legacy);
const listGraphql = read(paths.listGraphql);
const graphql = read(paths.graphql);
const graphqlHttp = read(paths.graphqlHttp);
const ui = read(paths.ui);
const priorGuard = read(paths.priorGuard);
const evidence = JSON.parse(read(paths.evidence));
const review = JSON.parse(read(paths.review));
const doc = read(paths.doc);
const masterPlan = read(paths.masterPlan);

requireText(cargo, "uuid.workspace = true", `${paths.cargo}: correlation UUID dependency`);
requireText(cargo, "tracing.workspace = true", `${paths.cargo}: private diagnostics dependency`);
requireText(
  facade,
  '#[path = "transport/graphql_error_safety.rs"]',
  `${paths.facade}: private policy module path`,
);
requireText(facade, "mod graphql_error_safety;", `${paths.facade}: private policy module`);
requireText(facade, "use rustok_graphql::GraphqlHttpError;", `${paths.facade}: typed error`);

const operations = [
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

for (const operation of operations) {
  const block = between(facade, operation.start, operation.end, paths.facade);
  for (const marker of [
    operation.context,
    operation.call,
    ".map_err(|error| context.map_error(error))",
  ]) {
    requireText(block, marker, `${paths.facade}: ${operation.name} final boundary`);
  }
  const contextIndex = block.indexOf(operation.context);
  const callIndex = block.indexOf(operation.call);
  if (contextIndex < 0 || callIndex < 0 || contextIndex > callIndex) {
    failures.push(`${paths.facade}: ${operation.name} context must precede the GraphQL call`);
  }
}

const shippingStart = facade.indexOf("pub(crate) async fn fetch_shipping_profiles(");
const shipping = shippingStart < 0 ? "" : facade.slice(shippingStart);
for (const marker of [
  "GraphqlReadContext::for_shipping_profiles(",
  "legacy::fetch_shipping_profiles(token, tenant_slug, tenant_id)",
  ".map_err(|error| context.map_error(error))",
]) {
  requireText(shipping, marker, `${paths.facade}: fetch_shipping_profiles final boundary`);
}
if (
  shipping.indexOf("GraphqlReadContext::for_shipping_profiles(") >
  shipping.indexOf("legacy::fetch_shipping_profiles(")
) {
  failures.push(`${paths.facade}: shipping context must precede the GraphQL call`);
}

if (countText(facade, ".map_err(|error| context.map_error(error))") !== 6) {
  failures.push(
    `${paths.facade}: expected five primary read mappers plus the retained catalog-options mapper`,
  );
}

const productList = operations.find((item) => item.name === "fetch_products");
const productListBlock = between(facade, productList.start, productList.end, paths.facade);
for (const marker of [
  "admin_catalog_native::fetch_products(",
  "Err(_) => {",
  "GraphqlReadContext::for_products(",
  "admin_catalog_graphql::fetch_products(",
]) {
  requireText(productListBlock, marker, `${paths.facade}: product-list native-first policy`);
}
const nativeIndex = productListBlock.indexOf("admin_catalog_native::fetch_products(");
const contextIndex = productListBlock.indexOf("GraphqlReadContext::for_products(");
const graphqlIndex = productListBlock.indexOf("admin_catalog_graphql::fetch_products(");
if (!(nativeIndex >= 0 && nativeIndex < contextIndex && contextIndex < graphqlIndex)) {
  failures.push(`${paths.facade}: product-list native/context/GraphQL order drift`);
}

for (const [marker, label] of [
  ["pub(super) struct GraphqlReadContext", "private typed context"],
  ["Uuid::new_v4()", "unique correlation id"],
  ['"product-admin-graphql:{operation}:{}"', "correlation namespace"],
  ["pub(super) fn map_error(&self, error: GraphqlHttpError)", "typed mapper"],
  ["GraphqlHttpError::Network", "network classification"],
  ["GraphqlHttpError::Http(_)", "HTTP classification"],
  ["GraphqlHttpError::Unauthorized", "authentication classification"],
  ["GraphqlHttpError::Graphql(_)", "GraphQL classification"],
  ['"Product admin service is temporarily unavailable"', "HTTP public message"],
  ['"Product admin request could not be completed"', "GraphQL public message"],
  ['"product.admin_graphql_network_unavailable"', "network code"],
  ['"product.admin_graphql_http_unavailable"', "HTTP code"],
  ['"product.admin_graphql_authentication_required"', "auth code"],
  ['"product.admin_graphql_request_rejected"', "GraphQL code"],
  ["raw_error = ?error", "private typed diagnostics"],
  ["correlation_id = %self.correlation_id", "correlation diagnostics"],
  ["token_present = self.token_present", "token presence"],
  ["tenant_slug_length = ?self.tenant_slug_length", "tenant slug shape"],
  ["tenant_id_length = ?self.tenant_id_length", "tenant ID shape"],
  ["resource_id_length = ?self.resource_id_length", "resource ID shape"],
  ["locale_length = ?self.locale_length", "locale shape"],
  ["search_length = ?self.search_length", "search shape"],
  ["status_length = ?self.status_length", "status shape"],
  ["currency_code_length = ?self.currency_code_length", "currency shape"],
  ["native_fallback_attempted = self.native_fallback_attempted", "fallback shape"],
  ["context.native_fallback_attempted = true;", "list fallback context"],
  ["public_error", "static typed return"],
]) {
  requireText(safety, marker, `${paths.safety}: ${label}`);
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
  "locale = %",
  "locale = ?",
  "search = %",
  "search = ?",
  "status = %",
  "status = ?",
  "currency_code = %",
  "currency_code = ?",
]) {
  forbidText(safety, marker, `${paths.safety}: raw request values must not be logged`);
}

for (const marker of [
  "graphql_adapter::fetch_bootstrap(token, tenant_slug).await",
  "graphql_adapter::fetch_product(token, tenant_slug, tenant_id, id, locale).await",
  "graphql_adapter::fetch_product_pricing(",
  "graphql_adapter::fetch_shipping_profiles(token, tenant_slug, tenant_id).await",
]) {
  requireText(legacy, marker, `${paths.legacy}: preserved private GraphQL delegation`);
}
for (const marker of ["ADMIN_PRODUCT_CATALOG_QUERY", "page: Some(1)", "per_page: Some(24)"]) {
  requireText(listGraphql, marker, `${paths.listGraphql}: preserved product-list query`);
}
for (const marker of [
  "pub type ApiError = GraphqlHttpError;",
  "BOOTSTRAP_QUERY",
  "PRODUCT_QUERY",
  "PRODUCT_PRICING_QUERY",
  "SHIPPING_PROFILES_QUERY",
]) {
  requireText(graphql, marker, `${paths.graphql}: preserved primary GraphQL contract`);
}
for (const marker of [
  "pub enum GraphqlHttpError",
  "Graphql(String)",
  "Http(String)",
  "Unauthorized",
]) {
  requireText(graphqlHttp, marker, `${paths.graphqlHttp}: typed GraphQL HTTP contract`);
}
for (const marker of [
  "transport::fetch_bootstrap(",
  "transport::fetch_products(",
  "transport::fetch_product(",
  "transport::fetch_product_pricing(",
  "transport::fetch_shipping_profiles(",
]) {
  requireText(ui, marker, `${paths.ui}: preserved UI resource composition`);
}
requireText(
  priorGuard,
  "Product Admin catalog search-options error-safety verification failed:",
  `${paths.priorGuard}: prior catalog-options guard remains present`,
);

if (evidence.schema_version !== 1) failures.push(`${paths.evidence}: schema_version must be 1`);
if (evidence.status !== "product_admin_primary_graphql_read_error_safety_source_unvalidated") {
  failures.push(`${paths.evidence}: status mismatch`);
}
if (
  JSON.stringify(evidence.operations) !==
  JSON.stringify([
    "fetch_bootstrap",
    "fetch_products",
    "fetch_product",
    "fetch_product_pricing",
    "fetch_shipping_profiles",
  ])
) {
  failures.push(`${paths.evidence}: operation scope drift`);
}
for (const [key, expected] of Object.entries({
  context_before_graphql_call: true,
  unique_correlation_id: true,
  typed_graphql_error_classification: true,
  network_static_public_envelope: true,
  http_static_public_envelope: true,
  unauthorized_static_public_envelope: true,
  graphql_static_public_envelope: true,
  raw_http_status_public: false,
  raw_graphql_message_public: false,
  private_typed_error_diagnostics: true,
  safe_request_shape_only: true,
  product_list_native_first_preserved: true,
  product_list_graphql_fallback_preserved: true,
  result_types_changed: false,
  graphql_documents_changed: false,
  graphql_variables_changed: false,
  response_mapping_changed: false,
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
  "product_admin_primary_graphql_read_error_safety_source_reviewed_unvalidated"
) {
  failures.push(`${paths.review}: status mismatch`);
}
requireText(doc, "Status: **source-ready / unvalidated**", `${paths.doc}: source status`);
requireText(doc, "Product admin service is temporarily unavailable", `${paths.doc}: HTTP policy`);
requireText(doc, "Product admin request could not be completed", `${paths.doc}: GraphQL policy`);
requireText(doc, "product-list native-first policy", `${paths.doc}: list policy`);
requireText(
  masterPlan,
  "Finish correlation-safe mapper cleanup",
  `${paths.masterPlan}: broad ecommerce mapper cleanup remains open`,
);

if (failures.length > 0) {
  console.error("Product Admin primary GraphQL read error-safety verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  "Product Admin primary GraphQL reads retain typed errors with correlation-safe static public payloads; execution evidence remains open",
);

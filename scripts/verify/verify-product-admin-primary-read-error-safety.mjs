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
  publicTransport: "crates/rustok-product/admin/src/catalog_transport.rs",
  safety: "crates/rustok-product/admin/src/transport/graphql_error_safety.rs",
  legacyTransport: "crates/rustok-product/admin/src/transport.rs",
  listGraphql: "crates/rustok-product/admin/src/transport/admin_catalog_graphql.rs",
  graphql: "crates/rustok-product/admin/src/transport/graphql_adapter.rs",
  graphqlHttp: "crates/rustok-graphql/src/lib.rs",
  ui: "crates/rustok-product/admin/src/ui/leptos.rs",
  evidence:
    "crates/rustok-product/contracts/evidence/admin-primary-graphql-read-error-safety-source.json",
  review:
    "crates/rustok-product/contracts/evidence/admin-primary-graphql-read-error-safety-source-review.json",
  doc: "crates/rustok-product/docs/admin-primary-graphql-read-error-safety.md",
  priorVerifier: "scripts/verify/verify-product-admin-catalog-options-error-safety.mjs",
  masterPlan: "crates/rustok-commerce/docs/implementation-plan.md",
};

const cargo = read(paths.cargo);
const publicTransport = read(paths.publicTransport);
const safety = read(paths.safety);
const legacyTransport = read(paths.legacyTransport);
const listGraphql = read(paths.listGraphql);
const graphql = read(paths.graphql);
const graphqlHttp = read(paths.graphqlHttp);
const ui = read(paths.ui);
const evidence = JSON.parse(read(paths.evidence));
const review = JSON.parse(read(paths.review));
const doc = read(paths.doc);
const priorVerifier = read(paths.priorVerifier);
const masterPlan = read(paths.masterPlan);

requireText(cargo, "uuid.workspace = true", `${paths.cargo}: correlation UUID dependency`);
requireText(cargo, "tracing.workspace = true", `${paths.cargo}: private diagnostics dependency`);

for (const marker of [
  '#[path = "transport/graphql_error_safety.rs"]',
  "mod graphql_error_safety;",
  "use rustok_graphql::GraphqlHttpError;",
  "pub(crate) async fn fetch_bootstrap(",
  "pub(crate) async fn fetch_products(",
  "pub(crate) async fn fetch_product(",
  "pub(crate) async fn fetch_product_pricing(",
  "pub(crate) async fn fetch_shipping_profiles(",
]) {
  requireText(publicTransport, marker, `${paths.publicTransport}: primary public read boundary`);
}

if (countText(publicTransport, ".map_err(|error| context.map_error(error))") !== 5) {
  failures.push(`${paths.publicTransport}: exactly five primary read mappings are required`);
}

const blocks = [
  {
    name: "fetch_bootstrap",
    source: between(
      publicTransport,
      "pub(crate) async fn fetch_bootstrap(",
      "pub async fn fetch_catalog_search_options(",
      paths.publicTransport,
    ),
    context: "GraphqlReadContext::for_bootstrap(",
    call: "legacy::fetch_bootstrap(token, tenant_slug)",
  },
  {
    name: "fetch_products",
    source: between(
      publicTransport,
      "pub(crate) async fn fetch_products(",
      "pub(crate) async fn fetch_product(",
      paths.publicTransport,
    ),
    context: "GraphqlReadContext::for_products(",
    call: "admin_catalog_graphql::fetch_products(",
  },
  {
    name: "fetch_product",
    source: between(
      publicTransport,
      "pub(crate) async fn fetch_product(",
      "pub(crate) async fn fetch_product_pricing(",
      paths.publicTransport,
    ),
    context: "GraphqlReadContext::for_product(",
    call: "legacy::fetch_product(token, tenant_slug, tenant_id, id, locale)",
  },
  {
    name: "fetch_product_pricing",
    source: between(
      publicTransport,
      "pub(crate) async fn fetch_product_pricing(",
      "pub(crate) async fn fetch_shipping_profiles(",
      paths.publicTransport,
    ),
    context: "GraphqlReadContext::for_product_pricing(",
    call: "legacy::fetch_product_pricing(",
  },
];

for (const block of blocks) {
  requireText(block.source, block.context, `${paths.publicTransport}: ${block.name} context`);
  requireText(block.source, block.call, `${paths.publicTransport}: ${block.name} adapter call`);
  requireText(
    block.source,
    ".map_err(|error| context.map_error(error))",
    `${paths.publicTransport}: ${block.name} final mapper`,
  );
  const contextIndex = block.source.indexOf(block.context);
  const callIndex = block.source.indexOf(block.call);
  if (contextIndex < 0 || callIndex < 0 || contextIndex > callIndex) {
    failures.push(`${paths.publicTransport}: ${block.name} context must precede the GraphQL call`);
  }
}

const shippingStart = publicTransport.indexOf("pub(crate) async fn fetch_shipping_profiles(");
const shipping = shippingStart < 0 ? "" : publicTransport.slice(shippingStart);
for (const marker of [
  "GraphqlReadContext::for_shipping_profiles(",
  "legacy::fetch_shipping_profiles(token, tenant_slug, tenant_id)",
  ".map_err(|error| context.map_error(error))",
]) {
  requireText(shipping, marker, `${paths.publicTransport}: shipping-profiles final mapper`);
}
const shippingContext = shipping.indexOf("GraphqlReadContext::for_shipping_profiles(");
const shippingCall = shipping.indexOf("legacy::fetch_shipping_profiles(");
if (shippingContext < 0 || shippingCall < 0 || shippingContext > shippingCall) {
  failures.push(`${paths.publicTransport}: shipping-profiles context must precede the GraphQL call`);
}

const products = blocks.find((block) => block.name === "fetch_products")?.source ?? "";
for (const marker of [
  "admin_catalog_native::fetch_products(",
  "Err(_) => {",
  "GraphqlReadContext::for_products(",
  "admin_catalog_graphql::fetch_products(",
  "context.native_fallback_attempted = true;",
]) {
  const source = marker === "context.native_fallback_attempted = true;" ? safety : products;
  requireText(source, marker, `${paths.publicTransport}: product-list native-first fallback`);
}
const nativeIndex = products.indexOf("admin_catalog_native::fetch_products(");
const contextIndex = products.indexOf("GraphqlReadContext::for_products(");
const graphqlIndex = products.indexOf("admin_catalog_graphql::fetch_products(");
if (
  nativeIndex < 0 ||
  contextIndex < 0 ||
  graphqlIndex < 0 ||
  !(nativeIndex < contextIndex && contextIndex < graphqlIndex)
) {
  failures.push(`${paths.publicTransport}: product-list native/context/GraphQL order drift`);
}

for (const [marker, label] of [
  ["pub(super) struct GraphqlReadContext", "private typed context"],
  ["Uuid::new_v4()", "unique correlation id"],
  ['"product-admin-graphql:{operation}:{}"', "correlation namespace"],
  ["pub(super) fn map_error(&self, error: GraphqlHttpError)", "typed final mapper"],
  ["GraphqlHttpError::Network", "network classification"],
  ["GraphqlHttpError::Http(_)", "HTTP classification"],
  ["GraphqlHttpError::Unauthorized", "authentication classification"],
  ["GraphqlHttpError::Graphql(_)", "GraphQL classification"],
  ['"Product admin service is temporarily unavailable"', "HTTP public message"],
  ['"Product admin request could not be completed"', "GraphQL public message"],
  ['"product.admin_graphql_network_unavailable"', "network stable code"],
  ['"product.admin_graphql_http_unavailable"', "HTTP stable code"],
  ['"product.admin_graphql_authentication_required"', "auth stable code"],
  ['"product.admin_graphql_request_rejected"', "GraphQL stable code"],
  ["raw_error = ?error", "private typed diagnostics"],
  ["correlation_id = %self.correlation_id", "correlation diagnostics"],
  ["token_present = self.token_present", "token presence diagnostics"],
  ["tenant_slug_length = ?self.tenant_slug_length", "tenant-slug shape"],
  ["tenant_id_length = ?self.tenant_id_length", "tenant-ID shape"],
  ["resource_id_length = ?self.resource_id_length", "resource-ID shape"],
  ["locale_length = ?self.locale_length", "locale shape"],
  ["search_length = ?self.search_length", "search shape"],
  ["status_length = ?self.status_length", "status shape"],
  ["currency_code_length = ?self.currency_code_length", "currency shape"],
  ["native_fallback_attempted = self.native_fallback_attempted", "fallback diagnostics"],
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
  "pub(crate) async fn fetch_bootstrap(",
  "graphql_adapter::fetch_bootstrap(token, tenant_slug).await",
  "pub(crate) async fn fetch_product(",
  "graphql_adapter::fetch_product(token, tenant_slug, tenant_id, id, locale).await",
  "pub(crate) async fn fetch_product_pricing(",
  "graphql_adapter::fetch_product_pricing(",
  "pub(crate) async fn fetch_shipping_profiles(",
  "graphql_adapter::fetch_shipping_profiles(token, tenant_slug, tenant_id).await",
]) {
  requireText(legacyTransport, marker, `${paths.legacyTransport}: preserved private adapter delegation`);
}
for (const marker of [
  "ADMIN_PRODUCT_CATALOG_QUERY",
  "page: Some(1)",
  "per_page: Some(24)",
]) {
  requireText(listGraphql, marker, `${paths.listGraphql}: preserved product-list GraphQL contract`);
}
for (const marker of [
  "pub type ApiError = GraphqlHttpError;",
  "BOOTSTRAP_QUERY",
  "PRODUCT_QUERY",
  "PRODUCT_PRICING_QUERY",
  "SHIPPING_PROFILES_QUERY",
]) {
  requireText(graphql, marker, `${paths.graphql}: preserved primary GraphQL documents`);
}
for (const marker of [
  "pub enum GraphqlHttpError",
  "Network",
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
  priorVerifier,
  "Product Admin catalog search-options error-safety verification failed:",
  `${paths.priorVerifier}: prior catalog-options guard remains present`,
);

if (evidence.schema_version !== 1) failures.push(`${paths.evidence}: schema_version must be 1`);
if (
  evidence.status !== "product_admin_primary_graphql_read_error_safety_source_unvalidated"
) {
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
requireText(doc, "product-list native-first policy", `${paths.doc}: preserved list policy`);
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

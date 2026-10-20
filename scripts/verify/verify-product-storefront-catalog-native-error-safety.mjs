#!/usr/bin/env node

import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const rootPath = configuredRoot
  ? path.resolve(configuredRoot)
  : fileURLToPath(new URL("../../", import.meta.url));
const read = (relativePath) => readFileSync(path.join(rootPath, relativePath), "utf8");

const cargo = read("crates/modules/rustok-product/storefront/Cargo.toml");
const source = read(
  "crates/modules/rustok-product/storefront/src/transport/catalog_list_native.rs",
);
const facets = read("crates/modules/rustok-product/storefront/src/transport/catalog_facets_native.rs");
const native = read("crates/modules/rustok-product/storefront/src/transport/native_server_adapter.rs");
const evidence = JSON.parse(
  read(
    "crates/modules/rustok-product/contracts/evidence/storefront-catalog-native-error-safety-source.json",
  ),
);

const failures = [];
const requireText = (content, value, label) => {
  if (!content.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (content, value, label) => {
  if (content.includes(value)) failures.push(`${label}: forbidden ${value}`);
};
const countText = (content, value) => content.split(value).length - 1;

// Product storefront logs GraphQL transport failures on every build profile
// (graphql_error_safety.rs), so tracing must stay an all-profile dependency —
// same convention as the pricing storefront gates.
requireText(cargo, "tracing.workspace = true", "Product storefront all-profile tracing dependency");
forbidText(cargo, '"dep:tracing"', "Product storefront stale SSR-only tracing feature");
forbidText(
  cargo,
  "tracing = { workspace = true, optional = true }",
  "Product storefront stale optional tracing dependency",
);

for (const [value, label] of [
  ["const PRODUCT_STOREFRONT_CATALOG_OWNER", "catalog owner constant"],
  ["const PRODUCT_STOREFRONT_CATALOG_OPERATION", "catalog operation constant"],
  ["const PRODUCT_STOREFRONT_CATALOG_BOUNDARY", "catalog boundary constant"],
  ["fn map_runtime_dependency_error(", "runtime dependency mapper"],
  ["fn map_tenant_context_error(request_context: &rustok_api::RequestContext)", "payload-free tenant context mapper"],
  ["owner = PRODUCT_STOREFRONT_CATALOG_OWNER", "owner diagnostics"],
  ["owner_operation = PRODUCT_STOREFRONT_CATALOG_OPERATION", "operation diagnostics"],
  ["boundary = PRODUCT_STOREFRONT_CATALOG_BOUNDARY", "boundary diagnostics"],
]) requireText(source, value, label);

for (const [value, label] of [
  ['endpoint = "product/storefront/catalog-list"', "catalog endpoint"],
  ["StorefrontProductListQuery::try_from_transport_with_attribute_filters(", "catalog query builder"],
  [".with_pagination(1, 12)", "catalog pagination"],
  ["currency_code: Option<String>", "catalog display-currency input"],
  [".with_currency_code(currency_code)", "catalog display-currency filter"],
  ["crate::core::resolve_requested_locale(", "locale fallback"],
  ["normalize_public_channel_slug(request_context.channel_slug.as_deref())", "channel fallback"],
  ["rustok_product::map_product_public_error(", "Product public error mapper"],
  ['map_product_service_error(error, "storefront_catalog_list_input")', "catalog input mapper operation"],
  ['map_product_service_error(error, "storefront_catalog_list")', "catalog service mapper operation"],
  ["native_server_adapter::fetch_products(request).await?", "detail transport composition"],
  ["data.products = products;", "catalog result composition"],
  ["use_context::<HostRuntimeContext>()", "fallible host runtime context lookup"],
  ['map_runtime_dependency_error("HostRuntimeContext")', "host runtime public safety mapper"],
]) requireText(source, value, label);

for (const [value, label] of [
  ["product.storefront_catalog_runtime_unavailable", "runtime stable code"],
  ["product.storefront_catalog_tenant_context_unavailable", "tenant context stable code"],
  ["Product catalog is temporarily unavailable", "runtime public message"],
  ["Product catalog context is unavailable", "context public message"],
]) requireText(source, value, label);

const compact = (value) => value.replace(/\s/g, "").replace(/,\)/g, ")");
function functionBody(text, name) {
  const match = new RegExp(`fn\\s+${name}\\s*\\(`).exec(text);
  if (!match) { failures.push(`missing native function ${name}`); return ""; }
  const start = text.indexOf("{", match.index);
  let depth = 0;
  for (let i = start; i < text.length; i++) {
    if (text[i] === "{") depth++;
    if (text[i] === "}" && --depth === 0) return text.slice(start, i + 1);
  }
  failures.push(`unterminated native function ${name}`); return "";
}
const required = functionBody(native, "require_request_context");
for (const value of [
  'leptos_axum::extract::<rustok_api::RequestContext>()',
  '.map_err(|_|',
  'ProductPublicError::internal()',
  'correlation_id = %public.correlation_id',
  'request_context_rejected = true',
  'ServerFnError::new(public.to_string())',
  'product.storefront_request_context_unavailable',
]) requireText(required, value, "required context rejection mapper");
for (const [text, fn, operation, boundary] of [
  [source, 'storefront_catalog_list_native', 'PRODUCT_STOREFRONT_CATALOG_OPERATION', 'PRODUCT_STOREFRONT_CATALOG_BOUNDARY'],
  [facets, 'storefront_catalog_facets_native', 'PRODUCT_STOREFRONT_FACETS_OPERATION', 'PRODUCT_STOREFRONT_FACETS_BOUNDARY'],
  [native, 'storefront_products_native', '"storefront_products"', '"product_storefront_native_server"'],
  [native, 'storefront_catalog_search_options_native', '"storefront_catalog_search_options"', '"product_storefront_native_server"'],
]) {
  const block = functionBody(text, fn);
  requireText(compact(block), compact(`require_request_context(${operation},${boundary},).await?;`), `${fn}: required context propagation`);
  const gate = block.indexOf('require_request_context(');
  const tenant = block.indexOf('extract::<rustok_api::TenantContext>()');
  const owner = block.indexOf('Service::new(');
  if (!(gate >= 0 && gate < tenant && tenant < owner)) failures.push(`${fn}: context gate must precede tenant and owner service`);
  for (const value of ['extract::<rustok_api::RequestContext>()', 'Err(_) => None', 'record_optional_request_context_error']) forbidText(block, value, `${fn}: rejected context must not become optional`);
}
for (const [text, label] of [[source, 'catalog list'], [facets, 'catalog facets']]) {
  const diagnostic = functionBody(text, 'map_tenant_context_error');
  for (const marker of [
    'request_context_present = true',
    'correlation_id_length = request_context.correlation_id.chars().count()',
    'channel_id_present = request_context.channel_id.is_some()',
    'channel_slug_length =', 'locale_length = request_context.locale.chars().count()',
  ]) requireText(diagnostic, marker, `${label}: bounded context facts`);
  requireText(compact(text), 'Some(request_context.locale.as_str())', `${label}: resolved locale preserved`);
  requireText(text, '.map_err(|_| map_tenant_context_error(&request_context))?', `${label}: mandatory tenant extraction`);
}
for (const [text, label] of [[source, 'catalog list'], [facets, 'catalog facets'], [required, 'required context helper']]) {
  for (const value of [
    'error = ?error', 'error = %error', 'std::fmt::Debug',
    'correlation_id = %request_context', 'channel_id = ?request_context',
    'channel_slug = ?request_context', 'locale = %request_context',
    'format!("{error', 'error.to_string()',
  ]) forbidText(text, value, `${label}: raw context diagnostic`);
}
if (countText(source, "map_product_service_error(error,") !== 3) {
  failures.push(
    "Product public mapper must remain on input, display-currency, and catalog service failures",
  );
}
if (countText(source, "ServerFnError::new(\"Product catalog is temporarily unavailable\")") !== 1) {
  failures.push("runtime composition must use exactly one stable catalog envelope");
}
if (countText(source, "ServerFnError::new(\"Product catalog context is unavailable\")") !== 1) {
  failures.push("tenant extraction must use exactly one stable context envelope");
}

forbidText(required, ".ok()", "required context rejection must propagate");

for (const value of [
  ".map_err(ServerFnError::new)?",
  "expect_context::<HostRuntimeContext>()",
  "product/storefront catalog list requires TransactionalEventBus in host runtime context",
  ".await\n            .ok();",
]) forbidText(source, value, "raw Product catalog native context mapping");

if (evidence.status !== "product_storefront_catalog_native_error_safety_source_unvalidated") {
  failures.push(`evidence status mismatch: ${evidence.status}`);
}
for (const [key, expected] of Object.entries({
  runtime_dependency_static_public_envelope: true,
  host_runtime_context_fallible: true,
  host_runtime_context_failure_public_safe: true,
  tenant_context_static_public_envelope: true,
  request_context_required: true,
  request_context_rejection_fail_closed: true,
  request_context_failure_logged: true,
  raw_context_payload_logged: false,
  correlation_shape_only: true,
  channel_shape_only: true,
  locale_shape_only: true,
  product_public_error_mapper_preserved: true,
  catalog_query_contract_changed: true,
  pagination_changed: false,
  locale_fallback_changed: false,
  channel_fallback_changed: false,
  request_response_dto_changed: true,
  raw_context_error_public: false,
})) {
  if (evidence.source_contract?.[key] !== expected) {
    failures.push(`evidence source_contract.${key} must be ${expected}`);
  }
}
for (const key of [
  "tests_run",
  "cargo_run",
  "workflow_checks_run",
  "ci_run",
  "native_runtime_proven",
  "mounted_parity_proven",
]) {
  if (evidence.validation?.[key] !== false) {
    failures.push(`evidence validation.${key} must remain false`);
  }
}

for (const key of ["format_run", "verifiers_run"]) {
  if (evidence.validation?.[key] !== true) failures.push(`evidence validation.${key} must record scoped source verification`);
}

if (failures.length > 0) {
  console.error("Product storefront catalog native error-safety verification failed:");
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  "✔ Product storefront native reads require trusted context, reject scope-widening fallback and retain bounded diagnostics; runtime evidence remains open",
);

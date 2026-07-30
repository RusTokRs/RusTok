#!/usr/bin/env node

import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const rootPath = configuredRoot
  ? path.resolve(configuredRoot)
  : fileURLToPath(new URL("../../", import.meta.url));
const read = (relativePath) => readFileSync(path.join(rootPath, relativePath), "utf8");

const cargo = read("crates/rustok-product/storefront/Cargo.toml");
const source = read(
  "crates/rustok-product/storefront/src/transport/catalog_list_native.rs",
);
const evidence = JSON.parse(
  read(
    "crates/rustok-product/contracts/evidence/storefront-catalog-native-error-safety-source.json",
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

for (const [value, label] of [
  ['"dep:tracing"', "Product storefront SSR tracing feature"],
  ["tracing = { workspace = true, optional = true }", "Product storefront tracing dependency"],
]) requireText(cargo, value, label);

for (const [value, label] of [
  ["const PRODUCT_STOREFRONT_CATALOG_OWNER", "catalog owner constant"],
  ["const PRODUCT_STOREFRONT_CATALOG_OPERATION", "catalog operation constant"],
  ["const PRODUCT_STOREFRONT_CATALOG_BOUNDARY", "catalog boundary constant"],
  ["fn map_runtime_dependency_error(", "runtime dependency mapper"],
  ["fn record_optional_request_context_error<E: std::fmt::Debug>(", "optional request context logger"],
  ["fn map_tenant_context_error<E: std::fmt::Debug>(", "tenant context mapper"],
  ["owner = PRODUCT_STOREFRONT_CATALOG_OWNER", "owner diagnostics"],
  ["owner_operation = PRODUCT_STOREFRONT_CATALOG_OPERATION", "operation diagnostics"],
  ["correlation_id = %request_context.correlation_id", "correlation diagnostics"],
  ["channel_id = ?request_context.channel_id", "channel id diagnostics"],
  ["channel_slug = ?request_context.channel_slug", "channel slug diagnostics"],
  ["locale = %request_context.locale", "locale diagnostics"],
  ["boundary = PRODUCT_STOREFRONT_CATALOG_BOUNDARY", "boundary diagnostics"],
  ["error = ?error", "internal context diagnostics"],
]) requireText(source, value, label);

for (const [value, label] of [
  ['endpoint = "product/storefront/catalog-list"', "catalog endpoint"],
  ["StorefrontProductListQuery::try_from_transport_with_attribute_filters(", "catalog query builder"],
  [".with_pagination(1, 12)", "catalog pagination"],
  ["crate::core::resolve_requested_locale(", "locale fallback"],
  ["normalize_public_channel_slug(context.channel_slug.as_deref())", "channel fallback"],
  ["rustok_product::map_product_public_error(", "Product public error mapper"],
  ['map_product_service_error(error, "storefront_catalog_list_input")', "catalog input mapper operation"],
  ['map_product_service_error(error, "storefront_catalog_list")', "catalog service mapper operation"],
  ["native_server_adapter::fetch_products(request).await?", "detail transport composition"],
  ["data.products = products;", "catalog result composition"],
]) requireText(source, value, label);

for (const [value, label] of [
  ["product.storefront_catalog_runtime_unavailable", "runtime stable code"],
  ["product.storefront_catalog_request_context_unavailable", "request context stable code"],
  ["product.storefront_catalog_tenant_context_unavailable", "tenant context stable code"],
  ["Product catalog is temporarily unavailable", "runtime public message"],
  ["Product catalog context is unavailable", "context public message"],
]) requireText(source, value, label);

if (countText(source, "request_context.as_ref()") !== 3) {
  failures.push("optional request context must remain in tenant, locale, and channel handling");
}
if (countText(source, "map_product_service_error(error,") !== 2) {
  failures.push("Product public mapper must remain on input and catalog service failures");
}
if (countText(source, "ServerFnError::new(\"Product catalog is temporarily unavailable\")") !== 1) {
  failures.push("runtime composition must use exactly one stable catalog envelope");
}
if (countText(source, "ServerFnError::new(\"Product catalog context is unavailable\")") !== 1) {
  failures.push("tenant extraction must use exactly one stable context envelope");
}

for (const value of [
  ".map_err(ServerFnError::new)?",
  "product/storefront catalog list requires TransactionalEventBus in host runtime context",
  ".await\n            .ok();",
]) forbidText(source, value, "raw Product catalog native context mapping");

if (evidence.status !== "product_storefront_catalog_native_error_safety_source_unvalidated") {
  failures.push(`evidence status mismatch: ${evidence.status}`);
}
for (const [key, expected] of Object.entries({
  runtime_dependency_static_public_envelope: true,
  tenant_context_static_public_envelope: true,
  optional_request_context_preserved: true,
  optional_request_context_failure_logged: true,
  product_public_error_mapper_preserved: true,
  catalog_query_contract_changed: false,
  pagination_changed: false,
  locale_fallback_changed: false,
  channel_fallback_changed: false,
  request_response_dto_changed: false,
  raw_context_error_public: false,
})) {
  if (evidence.source_contract?.[key] !== expected) {
    failures.push(`evidence source_contract.${key} must be ${expected}`);
  }
}
for (const key of [
  "tests_run",
  "cargo_run",
  "format_run",
  "verifiers_run",
  "workflow_checks_run",
  "ci_run",
  "native_runtime_proven",
  "mounted_parity_proven",
]) {
  if (evidence.validation?.[key] !== false) {
    failures.push(`evidence validation.${key} must remain false`);
  }
}

if (failures.length > 0) {
  console.error("Product storefront catalog native error-safety verification failed:");
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  "✔ Product storefront catalog native context failures use static public envelopes; runtime evidence remains open",
);

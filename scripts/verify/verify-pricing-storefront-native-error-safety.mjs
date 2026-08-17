#!/usr/bin/env node

import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const rootPath = configuredRoot
  ? path.resolve(configuredRoot)
  : fileURLToPath(new URL("../../", import.meta.url));
const read = (relativePath) => readFileSync(path.join(rootPath, relativePath), "utf8");

const cargo = read("crates/rustok-pricing/storefront/Cargo.toml");
const source = read(
  "crates/rustok-pricing/storefront/src/transport/native_server_adapter.rs",
);
const evidence = JSON.parse(
  read(
    "crates/rustok-pricing/contracts/evidence/storefront-native-error-safety-source.json",
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

requireText(cargo, "tracing.workspace = true", "Pricing storefront tracing dependency");
forbidText(cargo, '"dep:tracing"', "Pricing storefront stale SSR-only tracing feature");
forbidText(
  cargo,
  "tracing = { workspace = true, optional = true }",
  "Pricing storefront stale optional tracing dependency",
);

for (const [value, label] of [
  ["const PRICING_STOREFRONT_NATIVE_OWNER", "native owner constant"],
  ["const PRICING_STOREFRONT_NATIVE_BOUNDARY", "native boundary constant"],
  ["fn map_runtime_dependency_error(", "runtime dependency mapper"],
  ["fn record_optional_request_context_error<E: std::fmt::Display>(", "optional request context logger"],
  ["fn map_tenant_context_error<E: std::fmt::Display>(", "tenant context mapper"],
  ["fn map_owner_runtime_error<E: std::fmt::Display>(", "owner runtime mapper"],
  ["owner = PRICING_STOREFRONT_NATIVE_OWNER", "owner diagnostics"],
  ["owner_operation = operation", "owner operation diagnostics"],
  ["tenant_id = %tenant_id", "tenant diagnostics"],
  ["channel_id = ?request_context.channel_id", "channel id diagnostics"],
  ["channel_slug = ?request_context.channel_slug", "channel slug diagnostics"],
  ["locale = %request_context.locale", "locale diagnostics"],
  ["boundary = PRICING_STOREFRONT_NATIVE_BOUNDARY", "boundary diagnostics"],
  ["error = %error", "internal cause diagnostics"],
]) requireText(source, value, label);
forbidText(
  source,
  "request_context.correlation_id",
  "removed RequestContext correlation field",
);

for (const [value, label] of [
  ['endpoint = "pricing/storefront-data"', "pricing endpoint"],
  ["expect_context::<HostRuntimeContext>()", "host runtime composition"],
  ["shared_get::<TransactionalEventBus>()", "event-bus composition"],
  ["resolve_requested_locale(", "locale fallback"],
  ["normalize_public_channel_slug(ctx.channel_slug.as_deref())", "channel slug fallback"],
  ["sanitize_resolution_context(", "resolution-context validation"],
  [".list_channels(tenant.id, 1, 250)", "channel pagination"],
  [".list_active_price_lists_for_channel(", "active price-list operation"],
  [".list_published_product_pricing_with_locale_fallback(", "pricing list operation"],
  [".get_published_product_pricing_by_handle_with_locale_fallback(", "pricing detail operation"],
  [".resolve_variant_price(tenant.id, variant_id, context.clone())", "effective price operation"],
  ["Self::ServerFn(value.to_string())", "outer ServerFn error variant"],
  ["products: map_native_list(products)", "product result composition"],
  ["selected_product,", "selected product result composition"],
  ["resolution_context,", "resolution context result composition"],
  ["active_price_lists,", "price-list result composition"],
]) requireText(source, value, label);

for (const operation of [
  "list_channels",
  "list_active_price_lists_for_channel",
  "list_published_product_pricing_with_locale_fallback",
  "get_published_product_pricing_by_handle_with_locale_fallback",
  "parse_pricing_variant_id",
  "resolve_variant_price",
]) {
  requireText(source, `"${operation}"`, `owner operation ${operation}`);
}

for (const [value, label] of [
  ["pricing.storefront_runtime_unavailable", "runtime stable code"],
  ["pricing.storefront_request_context_unavailable", "request context stable code"],
  ["pricing.storefront_tenant_context_unavailable", "tenant context stable code"],
  ["pricing.storefront_owner_runtime_failed", "owner runtime stable code"],
  ["Storefront pricing is temporarily unavailable", "pricing unavailable public message"],
  ["Pricing storefront context is unavailable", "context unavailable public message"],
]) requireText(source, value, label);

if (countText(source, "map_owner_runtime_error") !== 7) {
  failures.push("owner runtime mapper must cover six operations plus its definition");
}
if (countText(source, ".map_err(|err| ServerFnError::new(err.to_string()))?;") !== 2) {
  failures.push("exactly two transport-validation mappings must remain user-facing");
}
if (countText(source, 'ServerFnError::new("Storefront pricing is temporarily unavailable")') !== 2) {
  failures.push("runtime dependency and owner failures must share two stable pricing envelope definitions");
}
if (countText(source, 'ServerFnError::new("Pricing storefront context is unavailable")') !== 1) {
  failures.push("tenant extraction must use exactly one stable context envelope definition");
}
if (countText(source, "Self::ServerFn(value.to_string())") !== 1) {
  failures.push("outer native facade must preserve exactly one ServerFn conversion");
}

for (const value of [
  "pricing/storefront-data requires TransactionalEventBus in host runtime context",
  ".map_err(ServerFnError::new)?",
  "Uuid::parse_str(&variant.id).map_err(ServerFnError::new)?",
]) forbidText(source, value, "raw Pricing storefront runtime mapping");

if (evidence.status !== "pricing_storefront_native_error_safety_source_unvalidated") {
  failures.push(`evidence status mismatch: ${evidence.status}`);
}
for (const [key, expected] of Object.entries({
  runtime_dependency_static_public_envelope: true,
  tenant_context_static_public_envelope: true,
  optional_request_context_preserved: true,
  optional_request_context_failure_logged: true,
  owner_runtime_static_public_envelope: true,
  internal_variant_id_failure_static_public_envelope: true,
  correlation_logging_when_available: true,
  transport_validation_messages_preserved: true,
  query_contract_changed: false,
  resolution_context_changed: false,
  pagination_changed: false,
  locale_fallback_changed: false,
  channel_fallback_changed: false,
  request_response_dto_changed: false,
  outer_error_variant_changed: false,
  raw_runtime_error_public: false,
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
  console.error("Pricing storefront native error-safety verification failed:");
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  "✔ Pricing storefront native runtime and owner failures use static public envelopes; runtime evidence remains open",
);

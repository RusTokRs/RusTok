#!/usr/bin/env node
// Source invariants only. Does not execute tests, native handlers, SQL or Rust compilation.
import { readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim()
  ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT)
  : fileURLToPath(new URL('../../', import.meta.url));
const read = (file) => readFileSync(path.join(root, file), 'utf8');
const source = read('crates/modules/rustok-pricing/storefront/src/transport/native_server_adapter.rs');
const cargo = read('crates/modules/rustok-pricing/storefront/Cargo.toml');
const evidence = JSON.parse(read('crates/modules/rustok-pricing/contracts/evidence/storefront-native-error-safety-source.json'));
const compact = (text) => text.replace(/\s+/g, '');
const failures = [];
const requireText = (text, value, label) => {
  if (!compact(text).includes(compact(value))) failures.push(`${label}: missing ${value}`);
};
const forbidText = (text, value, label) => {
  if (compact(text).includes(compact(value))) failures.push(`${label}: forbidden ${value}`);
};

requireText(cargo, 'tracing.workspace = true', 'all-profile tracing');
forbidText(cargo, 'tracing = { workspace = true, optional = true }', 'tracing must not disappear in CSR');
for (const marker of [
  'const PRICING_STOREFRONT_NATIVE_OWNER', 'const PRICING_STOREFRONT_NATIVE_BOUNDARY',
  'fn map_runtime_dependency_error(', 'fn map_request_context_error<',
  'fn map_tenant_context_error<', 'fn map_owner_runtime_error<',
  'owner = PRICING_STOREFRONT_NATIVE_OWNER', 'boundary = PRICING_STOREFRONT_NATIVE_BOUNDARY',
  'request_context_rejected = true',
  'leptos_axum::extract::<rustok_api::RequestContext>()', '.map_err(map_request_context_error)?',
  'endpoint = "pricing/storefront-data"', 'expect_context::<HostRuntimeContext>()',
  'shared_get::<TransactionalEventBus>()', 'in_process_pricing_read_port(',
  'normalize_pricing_context(', 'parse_optional_uuid_string(query.channel_id, "channel_id")',
  '.list_channels(tenant.id, 1, 250)',
  '.list_active_price_list_projections(', '.list_storefront_product_pricing(',
  '.read_storefront_product_pricing_projection(', '.resolve_product_prices(',
  'StorefrontProductPricingListRequest {', 'StorefrontProductPricingProjectionRequest {',
  'ResolveProductPricesRequest {', 'public_channel_id: selected_channel_id',
  'rustok_pricing::map_pricing_port_error(&error).to_string()',
  'quotes.remove(&variant.id)', 'snapshot.context', 'snapshot.resolved_at.to_rfc3339()',
  'value.source.price_id()', 'value.source.kind()', 'value.source.rule()',
  'products: map_native_list(products)', 'Self::ServerFn(value.to_string())',
  'Storefront pricing is temporarily unavailable', 'Pricing storefront context is unavailable',
]) requireText(source, marker, 'native Pricing owner boundary');
for (const marker of [
  'record_optional_request_context_error', 'PricingService::new(', '.resolve_variant_price(',
  '.list_published_product_pricing_with_locale_fallback(',
  '.get_published_product_pricing_by_handle_with_locale_fallback(',
  '.map_err(ServerFnError::new)?',
  'Uuid::parse_str(&variant.id).map_err(ServerFnError::new)?',
]) forbidText(source, marker, 'no bypass, context downgrade or raw public error');

if (evidence.status !== 'pricing_storefront_native_error_safety_source_unvalidated') {
  failures.push('native evidence must remain source-only / runtime unvalidated');
}
for (const [key, expected] of Object.entries({
  runtime_dependency_static_public_envelope: true,
  tenant_context_static_public_envelope: true,
  trusted_request_context_required: true,
  rejected_request_context_fails_closed: true,
  owner_runtime_static_public_envelope: true,
  internal_variant_id_failure_static_public_envelope: true,
  correlation_logging_when_available: true,
  transport_validation_messages_preserved: true,
  query_contract_changed: true,
  resolution_context_changed: true,
  pagination_changed: false,
  locale_fallback_changed: false,
  channel_fallback_changed: true,
  request_response_dto_changed: true,
  owner_read_port_batch: true,
  owner_catalog_projection: true,
  outer_error_variant_changed: false,
  raw_runtime_error_public: false,
})) {
  if (evidence.source_contract?.[key] !== expected) failures.push(`source_contract.${key} must be ${expected}`);
}
for (const key of ['tests_run', 'cargo_run', 'workflow_checks_run', 'ci_run', 'native_runtime_proven', 'mounted_parity_proven']) {
  if (evidence.validation?.[key] !== false) failures.push(`unrun runtime gate ${key} must remain false`);
}
for (const key of ['format_run', 'verifiers_run']) {
  if (typeof evidence.validation?.[key] !== 'boolean') failures.push(`validation.${key} must be an explicit static-check boolean`);
}
if (failures.length) {
  console.error('Pricing storefront native error-safety verification failed:');
  failures.forEach((failure) => console.error(`✗ ${failure}`));
  process.exit(Math.min(failures.length, 255));
}
console.log('✔ Pricing native reads use owner catalog/batch ports and fail closed on rejected context; runtime evidence remains open');

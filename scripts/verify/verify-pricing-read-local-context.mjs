#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const read = (relativePath) => readFileSync(new URL(relativePath, root), 'utf8');
const failures = [];

const lib = read('crates/rustok-pricing/src/lib.rs');
const legacy = read('crates/rustok-pricing/src/ports.rs');
const wrapper = read('crates/rustok-pricing/src/read_context.rs');

const requireText = (source, value, label) => {
  if (!source.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (source, value, label) => {
  if (source.includes(value)) failures.push(`${label}: forbidden ${value}`);
};

for (const [source, value, label] of [
  [lib, 'mod read_context;', 'private wrapper module'],
  [lib, 'pub use read_context::{InProcessPricingReadPort, in_process_pricing_read_port};', 'canonical root wrapper export'],
  [lib, 'in_process_pricing_write_port,', 'unchanged root write factory'],
  [legacy, 'pub fn in_process_pricing_read_port(', 'legacy compatibility factory'],
  [legacy, 'impl PricingReadPort for crate::PricingService', 'unchanged owner implementation'],
  [wrapper, 'pub struct InProcessPricingReadPort', 'canonical read wrapper'],
  [wrapper, 'pub fn from_service(inner: PricingService) -> Self', 'host composition constructor'],
  [wrapper, 'pub fn in_process_pricing_read_port(', 'canonical read factory'],
  [wrapper, 'Arc::new(InProcessPricingReadPort::new(db, event_bus))', 'wrapper factory construction'],
  [wrapper, 'impl PricingReadPort for InProcessPricingReadPort', 'wrapper trait implementation'],
  [wrapper, 'const PRICING_OWNER: &str = "rustok_pricing";', 'truthful owner'],
  [wrapper, 'const PRICING_READ_BOUNDARY: &str = "pricing_read_port";', 'stable boundary'],
]) {
  requireText(source, value, label);
}

forbidText(lib, 'pub use ports::*;', 'wildcard root compatibility export');
forbidText(
  lib,
  'pub use ports::in_process_pricing_read_port',
  'legacy read factory exported as canonical root',
);

const operations = [
  ['resolve_product_price', 'RESOLVE_PRODUCT_PRICE_OPERATION'],
  ['read_price_list_projection', 'READ_PRICE_LIST_PROJECTION_OPERATION'],
  ['list_active_price_list_projections', 'LIST_ACTIVE_PRICE_LIST_PROJECTIONS_OPERATION'],
  ['read_admin_product_pricing_projection', 'READ_ADMIN_PRODUCT_PRICING_PROJECTION_OPERATION'],
  ['read_storefront_product_pricing_projection', 'READ_STOREFRONT_PRODUCT_PRICING_PROJECTION_OPERATION'],
  ['preview_variant_discount', 'PREVIEW_VARIANT_DISCOUNT_OPERATION'],
];

for (const [operation, constant] of operations) {
  requireText(
    wrapper,
    `PricingReadPort::${operation}(`,
    `${operation} unchanged owner delegation`,
  );
  requireText(wrapper, constant, `${operation} stable operation constant`);
}
const innerDelegations = wrapper.match(/&self\.inner/g) ?? [];
if (innerDelegations.length !== operations.length) {
  failures.push(
    `expected ${operations.length} unchanged owner delegations, found ${innerDelegations.length}`,
  );
}

const retainedContext = [
  'correlation_id = %context.correlation_id',
  'tenant_id = %context.tenant_id',
  'actor = ?context.actor',
  'claim_count = context.claims.len()',
  'role_count = context.roles.len()',
  'channel = ?context.channel',
  'locale = %context.locale',
  'causation_id = ?context.causation_id',
  'traceparent = ?context.traceparent',
  'idempotency_key = ?context.idempotency_key',
  'deadline_ms = ?context.deadline_ms',
];
for (const value of retainedContext) {
  requireText(wrapper, value, 'complete delegated pricing context');
}

const safeFacts = [
  'product_id = ?facts.product_id',
  'variant_id = ?facts.variant_id',
  'region_id = ?facts.region_id',
  'channel_id = ?facts.channel_id',
  'price_list_id = ?facts.price_list_id',
  'selected_price_list_id = ?facts.selected_price_list_id',
  'quantity = ?facts.quantity',
  'currency_code_length = ?facts.currency_code_length',
  'channel_slug_length = ?facts.channel_slug_length',
  'locale_length = ?facts.locale_length',
  'fallback_locale_length = ?facts.fallback_locale_length',
  'handle_length = ?facts.handle_length',
  'public_channel_slug_length = ?facts.public_channel_slug_length',
];
for (const value of safeFacts) {
  requireText(wrapper, value, 'safe pricing request facts');
}

const sanitizedOutcomes = [
  ['pricing.tenant_id_invalid', 'pricing request context is invalid'],
  ['pricing.variant_product_mismatch', 'variant does not belong to the requested product'],
  ['pricing.price_not_found', 'price was not found'],
  ['pricing.price_list_not_found', 'price list was not found'],
  ['pricing.product_not_found', 'product was not found'],
  ['pricing.variant_not_found', 'variant was not found'],
  ['pricing.duplicate_handle', 'pricing handle is already in use'],
  ['pricing.duplicate_sku', 'pricing SKU is already in use'],
  ['pricing.insufficient_inventory', 'inventory is insufficient for the pricing operation'],
  ['pricing.shipping_profile_not_found', 'shipping profile was not found'],
  ['pricing.duplicate_shipping_profile_slug', 'shipping profile slug is already in use'],
];
for (const [code, message] of sanitizedOutcomes) {
  requireText(wrapper, `"${code}"`, `${code} classification`);
  requireText(wrapper, `Some("${message}")`, `${code} stable public message`);
}

for (const value of [
  'PortError::new(',
  'error.kind.clone()',
  'error.code.clone()',
  'error.retryable',
  'original_message_length = error.message.chars().count()',
  'return error;',
  'mapped_error',
  'PortErrorKind::Unavailable | PortErrorKind::Timeout | PortErrorKind::InvariantViolation',
  'tracing::error!',
  'tracing::warn!',
]) {
  requireText(wrapper, value, 'same envelope or safe-message mapping');
}

for (const value of [
  'handle = %',
  'handle = ?',
  'channel_slug = %',
  'channel_slug = ?',
  'public_channel_slug = %',
  'public_channel_slug = ?',
  'currency_code = %',
  'currency_code = ?',
  'discount_percent =',
  'amount =',
  'compare_at_amount =',
  'error = ?error',
  'internal_message = %error.message',
  'original_message =',
]) {
  forbidText(wrapper, value, 'raw pricing payload logging');
}

for (const value of [
  'format!("variant {variant_id} does not belong to product {product_id}")',
  'format!("price for variant {variant_id} was not found")',
  'format!("price list {} was not found", request.price_list_id)',
  'format!("product {id} not found")',
  'format!("variant {id} not found")',
  'format!("duplicate handle `{handle}` for locale `{locale}`")',
  'format!("duplicate sku `{sku}`")',
  'format!("insufficient inventory: requested {requested}, available {available}")',
  'format!("shipping profile {id} not found")',
  'format!("duplicate shipping profile slug `{slug}`")',
]) {
  forbidText(wrapper, value, 'dynamic canonical public message');
}

if (failures.length > 0) {
  console.error('Pricing read local context verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ canonical pricing reads retain delegated context and publish only stable local outcomes',
);

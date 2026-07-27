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
const wrapper = read('crates/rustok-pricing/src/write_context.rs');
const consumer = read('crates/rustok-commerce/src/graphql/mutations/pricing.rs');

const requireText = (source, value, label) => {
  if (!source.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (source, value, label) => {
  if (source.includes(value)) failures.push(`${label}: forbidden ${value}`);
};

for (const [source, value, label] of [
  [lib, 'mod write_context;', 'private write wrapper module'],
  [
    lib,
    'pub use write_context::{InProcessPricingWritePort, in_process_pricing_write_port};',
    'canonical root write wrapper export',
  ],
  [lib, 'InProcessPricingReadPort, in_process_pricing_read_port', 'preserved root read wrapper'],
  [legacy, 'pub fn in_process_pricing_write_port(', 'legacy compatibility factory'],
  [legacy, 'impl PricingWritePort for crate::PricingService', 'unchanged write owner implementation'],
  [wrapper, 'pub struct InProcessPricingWritePort', 'canonical write wrapper'],
  [wrapper, 'pub fn from_service(inner: PricingService) -> Self', 'host composition constructor'],
  [wrapper, 'pub fn in_process_pricing_write_port(', 'canonical write factory'],
  [wrapper, 'Arc::new(InProcessPricingWritePort::new(db, event_bus))', 'wrapper factory construction'],
  [wrapper, 'impl PricingWritePort for InProcessPricingWritePort', 'wrapper trait implementation'],
  [wrapper, 'const PRICING_OWNER: &str = "rustok_pricing";', 'truthful owner'],
  [wrapper, 'const PRICING_WRITE_BOUNDARY: &str = "pricing_write_port";', 'stable boundary'],
  [consumer, 'in_process_pricing_write_port,', 'mounted root write factory import'],
  [consumer, 'in_process_pricing_write_port(db.clone(), event_bus.clone())', 'mounted write construction'],
]) {
  requireText(source, value, label);
}

forbidText(
  lib,
  'pub use ports::in_process_pricing_write_port',
  'legacy write factory exported as canonical root',
);
forbidText(lib, 'pub use ports::*;', 'wildcard root compatibility export');

const operations = [
  ['upsert_variant_price', 'UPSERT_VARIANT_PRICE_OPERATION'],
  ['set_price_list_scope', 'SET_PRICE_LIST_SCOPE_OPERATION'],
  ['apply_variant_discount', 'APPLY_VARIANT_DISCOUNT_OPERATION'],
  ['set_price_list_percentage_rule', 'SET_PRICE_LIST_PERCENTAGE_RULE_OPERATION'],
];

for (const [operation, constant] of operations) {
  requireText(wrapper, `PricingWritePort::${operation}(`, `${operation} unchanged owner delegation`);
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
  requireText(wrapper, value, 'complete delegated pricing write context');
}

const safeFacts = [
  'variant_id = ?facts.variant_id',
  'price_list_id = ?facts.price_list_id',
  'channel_id = ?facts.channel_id',
  'min_quantity = ?facts.min_quantity',
  'max_quantity = ?facts.max_quantity',
  'currency_code_length = ?facts.currency_code_length',
  'channel_slug_length = ?facts.channel_slug_length',
  'fallback_locale_length = ?facts.fallback_locale_length',
  'compare_at_amount_present = ?facts.compare_at_amount_present',
  'adjustment_percent_present = ?facts.adjustment_percent_present',
];
for (const value of safeFacts) {
  requireText(wrapper, value, 'safe pricing write request facts');
}

const sanitizedOutcomes = [
  ['pricing.tenant_id_invalid', 'pricing request context is invalid'],
  ['pricing.actor_id_invalid', 'pricing write actor is invalid'],
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
  'public_message = %mapped_error.message',
  'PortErrorKind::Unavailable | PortErrorKind::Timeout | PortErrorKind::InvariantViolation',
  'tracing::error!',
  'tracing::warn!',
  'mapped_error\n}',
]) {
  requireText(wrapper, value, 'same envelope or safe-message mapping');
}

for (const value of [
  'currency_code = %',
  'currency_code = ?',
  'channel_slug = %',
  'channel_slug = ?',
  'fallback_locale = %',
  'fallback_locale = ?',
  'handle = %',
  'handle = ?',
  'sku = %',
  'sku = ?',
  'amount =',
  'compare_at_amount =',
  'discount_percent =',
  'adjustment_percent =',
  'error = ?error',
  'mapped_error = ?mapped_error',
  'internal_message = %error.message',
  'original_message =',
]) {
  forbidText(wrapper, value, 'raw pricing write payload logging');
}

for (const value of [
  'format!("product {id} not found")',
  'format!("variant {id} not found")',
  'format!("duplicate handle `{handle}` for locale `{locale}`")',
  'format!("duplicate sku `{sku}`")',
  'format!("insufficient inventory: requested {requested}, available {available}")',
  'format!("shipping profile {id} not found")',
  'format!("duplicate shipping profile slug `{slug}`")',
]) {
  forbidText(wrapper, value, 'dynamic canonical write public message');
}

for (const value of [
  '"port.idempotency_key_required"',
  '"port.deadline_required"',
]) {
  forbidText(wrapper, value, 'shared admission envelope reclassification');
}

if (failures.length > 0) {
  console.error('Pricing write local context verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ canonical pricing writes retain delegated context and publish only stable local outcomes',
);

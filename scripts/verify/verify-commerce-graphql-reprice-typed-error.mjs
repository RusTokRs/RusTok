#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const read = (relativePath) => readFileSync(new URL(relativePath, root), 'utf8');

const moduleSource = read('crates/rustok-commerce/src/graphql/mutations/mod.rs');
const layeredSource = read(
  'crates/rustok-commerce/src/graphql/mutations/layered_order_helpers.rs',
);
const typedSource = read(
  'crates/rustok-commerce/src/graphql/mutations/typed_reprice_helper.rs',
);
const compatibilitySource = read(
  'crates/rustok-commerce/src/graphql/mutations/safe_helpers.rs',
);
const cartMutationSource = read(
  'crates/rustok-commerce/src/graphql/mutations/cart.rs',
);
const failures = [];

const requireText = (source, value, label) => {
  if (!source.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (source, value, label) => {
  if (source.includes(value)) failures.push(`${label}: forbidden ${value}`);
};

for (const [value, label] of [
  [
    '#[path = "typed_reprice_helper.rs"]\nmod typed_reprice_helper;',
    'private typed reprice module routing',
  ],
  [
    'pub(crate) use super::typed_reprice_helper::reprice_storefront_cart_line_items;',
    'mounted typed reprice override',
  ],
  ['pub(crate) use super::safe_order_helpers_impl::*;', 'compatibility symbol parity'],
]) {
  requireText(`${moduleSource}\n${layeredSource}`, value, label);
}

requireText(
  compatibilitySource,
  'super::legacy_helpers::reprice_storefront_cart_line_items(',
  'retained private compatibility implementation',
);
requireText(
  cartMutationSource,
  'reprice_storefront_cart_line_items(',
  'mounted cart mutation reprice call',
);

for (const [value, label] of [
  ['PortContext, PortError, PortErrorKind', 'typed port error imports'],
  ['enum RepriceFailureSource {', 'typed failure source'],
  ['Pricing,', 'pricing source'],
  ['Cart,', 'cart source'],
  ['Self::Pricing => "rustok_pricing"', 'pricing owner'],
  ['Self::Cart => "rustok_cart"', 'cart owner'],
  ['Self::Pricing => "resolve_product_price"', 'pricing owner operation'],
  [
    'Self::Cart => "reprice_storefront_line_items"',
    'cart owner operation',
  ],
  ['RepriceFailure::pricing(error)', 'pricing typed mapping'],
  ['RepriceFailure::cart(error)', 'cart typed mapping'],
  [
    'PortErrorKind::Unavailable | PortErrorKind::Timeout | PortErrorKind::InvariantViolation',
    'technical severity classification',
  ],
  ['tracing::error!(', 'technical error severity'],
  ['tracing::warn!(', 'ordinary rejection severity'],
  [
    'const STOREFRONT_REPRICE_GRAPHQL_BOUNDARY: &str = "commerce_graphql_storefront_reprice";',
    'stable reprice boundary',
  ],
  ['async_graphql::Error::new("Cart pricing could not be refreshed")', 'public message'],
  ['extensions.set("code", "CART_REPRICE_FAILED")', 'public code'],
  ['extensions.set("retryable", true)', 'public retryability'],
  ['public_graphql_error()', 'single stable public envelope return'],
]) {
  requireText(typedSource, value, label);
}

for (const [value, label] of [
  ['context: &PortContext', 'retained delegated context'],
  ['correlation_id = %context.correlation_id', 'correlation context'],
  ['tenant_id = %context.tenant_id', 'tenant context'],
  ['actor_kind = ?context.actor.kind', 'actor kind context'],
  ['actor_id = %context.actor.id', 'actor identity context'],
  ['context_channel_length = ?context_channel_length', 'bounded channel context'],
  ['context_locale_length', 'bounded locale context'],
  ['causation_id_present = context.causation_id.is_some()', 'causation presence'],
  ['traceparent_present = context.traceparent.is_some()', 'trace presence'],
  ['idempotency_key_present = context.idempotency_key.is_some()', 'idempotency presence'],
  ['deadline_ms = ?context.deadline_ms', 'deadline context'],
  ['cart_id = %cart_id', 'cart identity'],
  ['line_item_id = ?line_item_id', 'line-item identity'],
  ['variant_id = ?variant_id', 'variant identity'],
  ['product_id = ?product_id', 'product identity'],
  ['requested_quantity = ?requested_quantity', 'quantity fact'],
  ['planned_update_count', 'planned update count'],
  ['cart_line_item_count', 'cart line-item count'],
  ['currency_code_length', 'bounded currency fact'],
  ['request_channel_slug_length = ?request_channel_slug_length', 'bounded request channel fact'],
  ['owner_code = %failure.error.code', 'typed owner code'],
  ['owner_kind = ?failure.error.kind', 'typed owner kind'],
  ['owner_retryable = failure.error.retryable', 'typed owner retryability'],
  ['public_code = "CART_REPRICE_FAILED"', 'public code diagnostic'],
  ['public_retryable = true', 'public retryability diagnostic'],
]) {
  requireText(typedSource, value, label);
}

for (const [value, label] of [
  [
    'super::cart::contextual_pricing_read_port(db.clone(), event_bus.clone())',
    'contextual pricing owner factory',
  ],
  [
    'super::cart_safe_helpers::build_storefront_pricing_context(',
    'preserved pricing request context builder',
  ],
  [
    'super::cart_safe_helpers::storefront_pricing_port_context(',
    'preserved pricing port context builder',
  ],
  [
    'super::cart_safe_helpers::storefront_cart_pricing_update(',
    'preserved pricing update builder',
  ],
  [
    'super::cart_safe_helpers::storefront_cart_port_context(',
    'preserved cart port context builder',
  ],
  ['CartStorefrontRepriceRequest {', 'typed cart reprice request'],
]) {
  requireText(typedSource, value, label);
}

const pricingCalls = typedSource.match(/\.resolve_product_price\(/g) ?? [];
if (pricingCalls.length !== 1) {
  failures.push(`expected one pricing owner call, found ${pricingCalls.length}`);
}
const cartCalls = typedSource.match(/\.reprice_storefront_line_items\(/g) ?? [];
if (cartCalls.length !== 1) {
  failures.push(`expected one cart owner call, found ${cartCalls.length}`);
}
const publicEnvelopeDefinitions = typedSource.match(/fn public_graphql_error\(\)/g) ?? [];
if (publicEnvelopeDefinitions.length !== 1) {
  failures.push(`expected one public envelope definition, found ${publicEnvelopeDefinitions.length}`);
}

for (const value of [
  'legacy_graphql_error(',
  'super::legacy_helpers::',
  'async_graphql::Error::new(error.message)',
  'async_graphql::Error::new(error.to_string())',
  'async_graphql::Error::new(format!("{error}"))',
  'format!("{error:?}")',
  'failure.error.message',
  'error.message',
  'currency_code = %',
  'currency_code = ?',
  'channel_slug = %',
  'channel_slug = ?',
  'locale = %',
  'locale = ?',
  'amount = %',
  'amount = ?',
  'metadata = %',
  'metadata = ?',
  'pricing_adjustment = %',
  'pricing_adjustment = ?',
]) {
  forbidText(typedSource, value, 'typed storefront reprice boundary');
}

if (failures.length > 0) {
  console.error('Commerce GraphQL typed reprice verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ mounted storefront cart reprice uses typed pricing/cart outcomes, retained context, and one stable GraphQL envelope',
);

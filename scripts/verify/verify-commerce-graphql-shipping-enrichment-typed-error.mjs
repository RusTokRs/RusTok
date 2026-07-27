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

const moduleSource = read('crates/rustok-commerce/src/graphql/mutations/mod.rs');
const layeredSource = read(
  'crates/rustok-commerce/src/graphql/mutations/layered_order_helpers.rs',
);
const contextSource = read(
  'crates/rustok-commerce/src/graphql/mutations/shipping_option_read_context.rs',
);
const typedSource = read(
  'crates/rustok-commerce/src/graphql/mutations/typed_shipping_enrichment_helper.rs',
);
const safeSource = read('crates/rustok-commerce/src/graphql/mutations/safe_helpers.rs');
const legacySource = read('crates/rustok-commerce/src/graphql/mutations/helpers.rs');
const projectionSource = read('crates/rustok-commerce/src/storefront_shipping.rs');
const ownerSource = read('crates/rustok-fulfillment/src/shipping_option_read.rs');

const requireText = (source, value, label) => {
  if (!source.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (source, value, label) => {
  if (source.includes(value)) failures.push(`${label}: forbidden ${value}`);
};
const between = (source, start, end, label) => {
  const startIndex = source.indexOf(start);
  const endIndex = source.indexOf(end, startIndex + start.length);
  if (startIndex < 0 || endIndex < 0) {
    failures.push(`${label}: unable to isolate source block`);
    return '';
  }
  return source.slice(startIndex, endIndex);
};

const publicEnvelope = between(
  typedSource,
  'fn public_graphql_error()',
  '#[allow(clippy::too_many_arguments)]\nfn shipping_enrichment_graphql_error(',
  'public GraphQL envelope',
);
const mapper = between(
  typedSource,
  'fn shipping_enrichment_graphql_error(',
  'pub(crate) async fn enrich_storefront_cart(',
  'typed enrichment mapper',
);
const mountedHelper = typedSource.slice(
  typedSource.indexOf('pub(crate) async fn enrich_storefront_cart('),
);

for (const [source, value, label] of [
  [
    moduleSource,
    '#[path = "shipping_option_read_context.rs"]\nmod shipping_option_read_context;',
    'private read context seam',
  ],
  [
    moduleSource,
    '#[path = "typed_shipping_enrichment_helper.rs"]\nmod typed_shipping_enrichment_helper;',
    'private typed helper module',
  ],
  [
    layeredSource,
    'pub(crate) use super::typed_shipping_enrichment_helper::enrich_storefront_cart;',
    'mounted typed override',
  ],
  [safeSource, 'pub(crate) async fn enrich_storefront_cart(', 'compatibility facade'],
  [safeSource, 'super::legacy_helpers::enrich_storefront_cart(', 'compatibility delegation'],
  [legacySource, 'pub(crate) async fn enrich_storefront_cart(', 'legacy helper'],
  [
    projectionSource,
    'pub fn enrich_cart_delivery_groups_from_options(',
    'pure delivery-group projection',
  ],
  [ownerSource, 'pub trait ShippingOptionReadPort: Send + Sync {', 'owner read port'],
]) {
  requireText(source, value, label);
}

for (const [value, label] of [
  ['enum ShippingEnrichmentFailureKind {', 'typed outcome enum'],
  ['Validation,', 'validation outcome'],
  ['NotFound,', 'not-found outcome'],
  ['Conflict,', 'conflict outcome'],
  ['Forbidden,', 'forbidden outcome'],
  ['StorageUnavailable,', 'availability outcome'],
  ['Invariant,', 'invariant outcome'],
  ['owner_error: PortError', 'typed owner cause'],
  ['PortErrorKind::Validation', 'validation mapping'],
  ['PortErrorKind::NotFound', 'not-found mapping'],
  ['PortErrorKind::Conflict', 'conflict mapping'],
  ['PortErrorKind::Forbidden', 'forbidden mapping'],
  ['PortErrorKind::Unavailable | PortErrorKind::Timeout', 'availability mapping'],
  ['PortErrorKind::InvariantViolation', 'invariant mapping'],
]) {
  requireText(typedSource, value, label);
}

for (const [value, label] of [
  [
    'async_graphql::Error::new("Cart shipping details are temporarily unavailable")',
    'stable public message',
  ],
  ['extensions.set("code", "CART_ENRICHMENT_UNAVAILABLE")', 'stable public code'],
  ['extensions.set("retryable", true)', 'stable public retryability'],
]) {
  requireText(publicEnvelope, value, label);
}

for (const [value, label] of [
  ['context: &PortContext', 'retained owner context'],
  ['correlation_id = %context.correlation_id', 'correlation context'],
  ['tenant_id = %context.tenant_id', 'tenant context'],
  ['actor = ?context.actor', 'actor context'],
  ['context_channel_length = context.channel.as_deref().map(str::len)', 'bounded channel context'],
  ['context_locale_length = context.locale.len()', 'bounded locale context'],
  ['deadline_ms = ?context.deadline_ms', 'deadline context'],
  ['error = ?technical_owner_error', 'technical typed cause'],
  ['owner = "rustok_fulfillment"', 'truthful owner'],
  [
    'owner_operation = "list_shipping_option_projections"',
    'truthful owner operation',
  ],
  ['internal_code = %failure.internal_code', 'internal code'],
  ['internal_kind = failure.internal_kind', 'internal kind'],
  ['internal_retryable = failure.internal_retryable', 'internal retryability'],
  ['cart_id = %cart_id', 'cart identity'],
  ['line_item_count,', 'line item count'],
  ['delivery_group_count,', 'delivery group count'],
  ['currency_code_length,', 'currency length'],
  ['public_code = "CART_ENRICHMENT_UNAVAILABLE"', 'public code diagnostic'],
  ['public_retryable = true', 'public retryability diagnostic'],
  ['tracing::error!(', 'technical severity'],
  ['tracing::warn!(', 'ordinary severity'],
  ['public_graphql_error()', 'stable envelope return'],
]) {
  requireText(mapper, value, label);
}

for (const [value, label] of [
  ['let cart_id = cart.id;', 'cart identity retained before move'],
  ['let line_item_count = cart.line_items.len();', 'line item fact retained'],
  ['let delivery_group_count = cart.delivery_groups.len();', 'delivery group fact retained'],
  ['let currency_code_length = cart.currency_code.chars().count();', 'currency length retained'],
  ['normalize_public_channel_slug(cart.channel_slug.as_deref())', 'cart channel normalization'],
  ['normalize_public_channel_slug(request_context.channel_slug.as_deref())', 'request channel fallback'],
  ['storefront_shipping_option_read_context(', 'owner context construction'],
  ['storefront_shipping_option_read_port(db.clone())', 'owner port construction'],
  ['ListShippingOptionProjectionsRequest {', 'typed owner request'],
  ['.list_shipping_option_projections(', 'owner list call'],
  ['owner_context.clone(),', 'delegated owner context'],
  ['ShippingEnrichmentFailure::from_owner(error)', 'typed owner mapping'],
  ['enrich_cart_delivery_groups_from_options(', 'pure projection delegation'],
]) {
  requireText(mountedHelper, value, label);
}

const ownerCalls = mountedHelper.match(/\.list_shipping_option_projections\(/g) ?? [];
if (ownerCalls.length !== 1) {
  failures.push(`expected one shipping-option owner read, found ${ownerCalls.length}`);
}
const projectionCalls = mountedHelper.match(/enrich_cart_delivery_groups_from_options\(/g) ?? [];
if (projectionCalls.length !== 1) {
  failures.push(`expected one pure delivery-group projection, found ${projectionCalls.length}`);
}
const mountedOverrides = layeredSource.match(/enrich_storefront_cart/g) ?? [];
if (mountedOverrides.length !== 1) {
  failures.push(`expected one mounted enrichment override, found ${mountedOverrides.length}`);
}

for (const [value, label] of [
  ['PortActor::service("rustok-commerce.storefront-shipping")', 'service actor'],
  ['.with_deadline(std::time::Duration::from_secs(2))', 'read deadline'],
  [
    'rustok_fulfillment::in_process_shipping_option_read_port(db)',
    'canonical root factory',
  ],
]) {
  requireText(contextSource, value, label);
}

for (const value of [
  'FulfillmentService::new(',
  'FulfillmentError',
  'enrich_cart_delivery_groups_typed(',
  'enrich_cart_delivery_groups(',
  'CommerceError::Validation(error.to_string())',
  'async_graphql::Error::new(error.to_string())',
  'async_graphql::Error::new(format!("{error}"))',
  'format!("{error:?}")',
  'detail.contains(',
  'error.message',
  'error = ?failure.owner_error',
  'public_channel_slug = %',
  'public_channel_slug = ?',
  'requested_locale = %',
  'requested_locale = ?',
  'tenant_default_locale = %',
  'tenant_default_locale = ?',
  'currency_code = %',
  'currency_code = ?',
]) {
  forbidText(typedSource, value, 'typed storefront shipping enrichment boundary');
}

if (failures.length > 0) {
  console.error('Commerce GraphQL typed shipping enrichment verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ mounted storefront shipping enrichment uses the fulfillment read port, retained context, pure projection, and one stable GraphQL envelope',
);

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
const typedSource = read(
  'crates/rustok-commerce/src/graphql/mutations/typed_shipping_enrichment_helper.rs',
);
const safeSource = read('crates/rustok-commerce/src/graphql/mutations/safe_helpers.rs');
const legacySource = read('crates/rustok-commerce/src/graphql/mutations/helpers.rs');
const ownerSource = read('crates/rustok-commerce/src/storefront_shipping.rs');

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
    '#[path = "typed_shipping_enrichment_helper.rs"]\nmod typed_shipping_enrichment_helper;',
    'private typed helper module',
  ],
  [
    layeredSource,
    'pub(crate) use super::typed_shipping_enrichment_helper::enrich_storefront_cart;',
    'mounted typed override',
  ],
  [
    moduleSource,
    '#[allow(dead_code)]\n#[path = "safe_helpers.rs"]\nmod cart_safe_helpers;',
    'private compatibility facade allowance',
  ],
  [
    moduleSource,
    '#[allow(dead_code)]\n#[path = "safe_legacy_helpers.rs"]\nmod legacy_helpers;',
    'private legacy helper allowance',
  ],
  [safeSource, 'pub(crate) async fn enrich_storefront_cart(', 'compatibility facade'],
  [safeSource, 'super::legacy_helpers::enrich_storefront_cart(', 'compatibility delegation'],
  [legacySource, 'pub(crate) async fn enrich_storefront_cart(', 'legacy helper'],
  [ownerSource, 'pub async fn enrich_cart_delivery_groups_typed(', 'typed owner adapter'],
]) {
  requireText(source, value, label);
}

for (const [value, label] of [
  ['enum ShippingEnrichmentFailureKind {', 'typed outcome enum'],
  ['Validation,', 'validation outcome'],
  ['ShippingOptionNotFound,', 'option not-found outcome'],
  ['FulfillmentNotFound,', 'fulfillment not-found outcome'],
  ['LifecycleConflict,', 'lifecycle outcome'],
  ['StorageUnavailable,', 'storage outcome'],
  ['owner_error: FulfillmentError', 'typed owner cause'],
  ['FulfillmentError::Validation(_)', 'owner validation mapping'],
  ['FulfillmentError::ShippingOptionNotFound(_)', 'owner option mapping'],
  ['FulfillmentError::FulfillmentNotFound(_)', 'owner fulfillment mapping'],
  ['FulfillmentError::InvalidTransition { .. }', 'owner transition mapping'],
  ['FulfillmentError::Database(_)', 'owner database mapping'],
  ['"fulfillment.validation"', 'validation internal code'],
  ['"fulfillment.shipping_option_not_found"', 'option internal code'],
  ['"fulfillment.fulfillment_not_found"', 'fulfillment internal code'],
  ['"fulfillment.invalid_transition"', 'transition internal code'],
  ['"fulfillment.database_unavailable"', 'database internal code'],
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
  ['error = ?technical_owner_error', 'technical typed cause'],
  ['owner = "rustok_fulfillment"', 'truthful owner'],
  ['owner_operation = "list_shipping_options"', 'truthful owner operation'],
  ['internal_code = failure.internal_code', 'internal code'],
  ['internal_kind = failure.internal_kind', 'internal kind'],
  ['internal_retryable = failure.internal_retryable', 'internal retryability'],
  ['tenant_id = %tenant_id', 'tenant context'],
  ['cart_id = %cart_id', 'cart identity'],
  ['line_item_count,', 'line item count'],
  ['delivery_group_count,', 'delivery group count'],
  ['currency_code_length,', 'currency length'],
  ['channel_slug_length = ?channel_slug_length', 'channel length'],
  ['requested_locale_length = ?requested_locale_length', 'requested locale length'],
  ['tenant_default_locale_length = ?tenant_default_locale_length', 'default locale length'],
  ['public_code = "CART_ENRICHMENT_UNAVAILABLE"', 'public code diagnostic'],
  ['public_retryable = true', 'public retryability diagnostic'],
  ['boundary = STOREFRONT_SHIPPING_ENRICHMENT_GRAPHQL_BOUNDARY', 'stable boundary'],
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
  ['enrich_cart_delivery_groups_typed(', 'direct typed owner delegation'],
  ['Some(request_context.locale.as_str())', 'requested locale delegation'],
  ['Some(tenant_default_locale)', 'default locale delegation'],
  ['ShippingEnrichmentFailure::from_owner(error)', 'typed owner mapping'],
]) {
  requireText(mountedHelper, value, label);
}

const typedCalls = mountedHelper.match(/enrich_cart_delivery_groups_typed\(/g) ?? [];
if (typedCalls.length !== 1) {
  failures.push(`expected one direct typed enrichment call, found ${typedCalls.length}`);
}
const mountedOverrides = layeredSource.match(/enrich_storefront_cart/g) ?? [];
if (mountedOverrides.length !== 1) {
  failures.push(`expected one mounted enrichment override, found ${mountedOverrides.length}`);
}

for (const value of [
  'enrich_cart_delivery_groups(',
  'CommerceError::Validation(error.to_string())',
  'async_graphql::Error::new(error.to_string())',
  'async_graphql::Error::new(format!("{error}"))',
  'format!("{error:?}")',
  'detail.contains(',
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
  '✔ mounted storefront shipping enrichment delegates typed fulfillment outcomes to one stable GraphQL envelope',
);

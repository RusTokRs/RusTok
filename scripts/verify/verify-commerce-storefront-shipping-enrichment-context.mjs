#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

import { readCommerceSafeQuerySource } from './lib/commerce-safe-query-source.mjs';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const read = (relativePath) => readFileSync(new URL(relativePath, root), 'utf8');

const shipping = read('crates/rustok-commerce/src/storefront_shipping.rs');
const helper = read('crates/rustok-commerce/src/graphql/mutations/helpers.rs');
const helperFacade = read('crates/rustok-commerce/src/graphql/mutations/safe_helpers.rs');
const query = read('crates/rustok-commerce/src/graphql/query.rs');
const queryFacade = readCommerceSafeQuerySource(read);
const failures = [];

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

const compatibilityWrapper = between(
  shipping,
  'pub async fn enrich_cart_delivery_groups(',
  'fn log_cart_delivery_group_enrichment_error(',
  'shipping compatibility wrapper',
);
const diagnostic = between(
  shipping,
  'fn log_cart_delivery_group_enrichment_error(',
  'fn extract_allowed_shipping_profile_slugs_from_metadata(',
  'shipping enrichment diagnostic',
);

for (const [value, label] of [
  ['FulfillmentError, FulfillmentResult, FulfillmentService', 'typed fulfillment import'],
  ['const STOREFRONT_SHIPPING_ENRICHMENT_BOUNDARY: &str =', 'shared enrichment boundary'],
  ['"commerce_storefront_shipping_enrichment"', 'stable enrichment boundary value'],
  ['struct StorefrontShippingDiagnosticError;', 'redacted diagnostic token'],
  ['formatter.write_str("redacted")', 'redacted diagnostic output'],
  ['fn uuid_shape(value: Uuid)', 'UUID shape helper'],
  ['fn optional_text_shape(value: Option<&str>)', 'optional text shape helper'],
  ['enrich_cart_delivery_groups_typed(', 'typed enrichment delegation'],
  ['log_cart_delivery_group_enrichment_error(', 'typed cause logger'],
  ['FulfillmentError::Validation(_)', 'validation classification'],
  ['FulfillmentError::ShippingOptionNotFound(_)', 'shipping option classification'],
  ['FulfillmentError::FulfillmentNotFound(_)', 'fulfillment classification'],
  ['FulfillmentError::InvalidTransition { .. }', 'transition classification'],
  ['FulfillmentError::Database(_)', 'database classification'],
  ['owner = "rustok_fulfillment"', 'truthful owner identity'],
  ['tenant_id_shape,', 'bounded tenant context'],
  ['cart_id_shape,', 'bounded cart context'],
  ['public_channel_slug_shape,', 'bounded channel context'],
  ['requested_locale_shape,', 'bounded requested locale context'],
  ['tenant_default_locale_shape,', 'bounded default locale context'],
  ['operation = "list_shipping_options"', 'exact owner operation'],
  ['owner_code,', 'stable owner code'],
  ['owner_kind,', 'owner error kind'],
  ['owner_retryable,', 'owner retryability'],
  ['tracing::error!(', 'technical severity'],
  ['tracing::warn!(', 'ordinary rejection severity'],
  ['boundary = STOREFRONT_SHIPPING_ENRICHMENT_BOUNDARY', 'boundary logging'],
]) {
  requireText(shipping, value, label);
}

for (const [value, label] of [
  ['-> CommerceResult<CartResponse>', 'compatibility return contract'],
  ['enrich_cart_delivery_groups_typed(', 'compatibility typed delegation'],
  ['log_cart_delivery_group_enrichment_error(', 'compatibility diagnostic call'],
  [
    'crate::CommerceError::Validation(\n            "Cart shipping details are temporarily unavailable".to_string(),\n        )',
    'stable compatibility public envelope',
  ],
]) {
  requireText(compatibilityWrapper, value, label);
}

for (const value of [
  'error.to_string()',
  'err.to_string()',
  'format!("{error:?}")',
]) {
  forbidText(compatibilityWrapper, value, 'unsafe compatibility public conversion');
}
for (const value of [
  'tenant_id = %tenant_id',
  'cart_id = %cart_id',
  'public_channel_slug = ?public_channel_slug',
  'requested_locale = ?requested_locale',
  'tenant_default_locale = ?tenant_default_locale',
]) {
  forbidText(diagnostic, value, 'raw shipping enrichment diagnostic context');
}

const typedCalls = shipping.match(/enrich_cart_delivery_groups_typed\(/g) ?? [];
if (typedCalls.length !== 2) {
  failures.push(
    `expected typed enrichment definition plus one compatibility call, found ${typedCalls.length}`,
  );
}
const compatibilityMappers =
  compatibilityWrapper.match(/crate::CommerceError::Validation\(/g) ?? [];
if (compatibilityMappers.length !== 1) {
  failures.push(`expected one stable compatibility mapper, found ${compatibilityMappers.length}`);
}

for (const [source, value, label] of [
  [helper, 'enrich_cart_delivery_groups(', 'legacy mutation helper enrichment call'],
  [helperFacade, '"Cart shipping details are temporarily unavailable"', 'mutation public message'],
  [helperFacade, '"CART_ENRICHMENT_UNAVAILABLE"', 'mutation public code'],
  [query, 'let cart = enrich_cart_delivery_groups(', 'storefront cart query enrichment call'],
  [queryFacade, 'impl From<crate::CommerceError> for BoundaryError', 'query compatibility boundary'],
  [queryFacade, '"COMMERCE_QUERY_TEMPORARILY_UNAVAILABLE"', 'query unavailable code'],
  [queryFacade, '"COMMERCE_QUERY_OPERATION_FAILED"', 'query fail-closed code'],
]) {
  requireText(source, value, label);
}

if (failures.length > 0) {
  console.error('Commerce storefront shipping enrichment context verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Shared storefront cart shipping enrichment retains typed owner diagnostics and a stable compatibility envelope',
);

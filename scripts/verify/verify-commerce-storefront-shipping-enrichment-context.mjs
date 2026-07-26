#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const read = (relativePath) => readFileSync(new URL(relativePath, root), 'utf8');

const shipping = read('crates/rustok-commerce/src/storefront_shipping.rs');
const helper = read('crates/rustok-commerce/src/graphql/mutations/helpers.rs');
const helperFacade = read('crates/rustok-commerce/src/graphql/mutations/safe_helpers.rs');
const query = read('crates/rustok-commerce/src/graphql/query.rs');
const queryFacade = read('crates/rustok-commerce/src/graphql/safe_query.rs');
const failures = [];

const requireText = (source, value, label) => {
  if (!source.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (source, value, label) => {
  if (source.includes(value)) failures.push(`${label}: forbidden ${value}`);
};

for (const [value, label] of [
  ['FulfillmentError, FulfillmentResult, FulfillmentService', 'typed fulfillment import'],
  ['const STOREFRONT_SHIPPING_ENRICHMENT_BOUNDARY: &str =', 'shared enrichment boundary'],
  ['"commerce_storefront_shipping_enrichment"', 'stable enrichment boundary value'],
  ['let cart_id = cart.id;', 'cart identity retained before move'],
  ['enrich_cart_delivery_groups_typed(', 'typed enrichment delegation'],
  ['log_cart_delivery_group_enrichment_error(', 'typed cause logger'],
  ['FulfillmentError::Validation(_)', 'validation classification'],
  ['FulfillmentError::ShippingOptionNotFound(_)', 'shipping option classification'],
  ['FulfillmentError::FulfillmentNotFound(_)', 'fulfillment classification'],
  ['FulfillmentError::InvalidTransition { .. }', 'transition classification'],
  ['FulfillmentError::Database(_)', 'database classification'],
  ['owner = "rustok_fulfillment"', 'truthful owner identity'],
  ['tenant_id = %tenant_id', 'tenant context'],
  ['cart_id = %cart_id', 'cart context'],
  ['public_channel_slug = ?public_channel_slug', 'channel context'],
  ['requested_locale = ?requested_locale', 'requested locale context'],
  ['tenant_default_locale = ?tenant_default_locale', 'default locale context'],
  ['operation = "list_shipping_options"', 'exact owner operation'],
  ['owner_code,', 'stable owner code'],
  ['owner_kind,', 'owner error kind'],
  ['owner_retryable,', 'owner retryability'],
  ['boundary = STOREFRONT_SHIPPING_ENRICHMENT_BOUNDARY', 'boundary logging'],
  ['crate::CommerceError::Validation(error.to_string())', 'compatibility error preservation'],
]) {
  requireText(shipping, value, label);
}

const typedCalls = shipping.match(/enrich_cart_delivery_groups_typed\(/g) ?? [];
if (typedCalls.length !== 2) {
  failures.push(`expected typed enrichment definition plus one compatibility call, found ${typedCalls.length}`);
}
const compatibilityMappers =
  shipping.match(/crate::CommerceError::Validation\(error\.to_string\(\)\)/g) ?? [];
if (compatibilityMappers.length !== 1) {
  failures.push(`expected one unchanged compatibility mapper, found ${compatibilityMappers.length}`);
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

for (const value of [
  'async_graphql::Error::new(error.to_string())',
  'async_graphql::Error::new(format!("{error}"))',
]) {
  forbidText(shipping, value, 'shared shipping enrichment public conversion');
}

if (failures.length > 0) {
  console.error('Commerce storefront shipping enrichment context verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Shared storefront cart shipping enrichment retains typed fulfillment owner diagnostics before the existing compatibility and GraphQL envelopes',
);

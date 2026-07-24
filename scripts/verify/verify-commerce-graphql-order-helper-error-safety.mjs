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
const facadeSource = read('crates/rustok-commerce/src/graphql/mutations/safe_order_helpers.rs');
const failures = [];

const requireText = (source, value, label) => {
  if (!source.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (source, value, label) => {
  if (source.includes(value)) failures.push(`${label}: forbidden ${value}`);
};

for (const [value, label] of [
  ['#[path = "safe_helpers.rs"]\nmod cart_safe_helpers;', 'private cart facade routing'],
  ['#[path = "safe_order_helpers.rs"]\npub mod helpers;', 'public order facade routing'],
]) {
  requireText(moduleSource, value, label);
}

for (const value of [
  'async_graphql::Error::new(error.message)',
  'async_graphql::Error::new(error.to_string())',
  'async_graphql::Error::new(format!("{error}"))',
  'ensure_storefront_order_access,',
  'validate_product_shipping_profile_input,',
  'validate_shipping_option_profile_inputs,',
]) {
  forbidText(facadeSource, value, 'order and shipping safe helper facade');
}

for (const [value, label] of [
  ['fn order_graphql_error(', 'order mapper'],
  ['OrderError::Validation(_)', 'order validation mapping'],
  ['OrderError::OrderNotFound(_)', 'order not-found mapping'],
  ['OrderError::OrderReturnNotFound(_)', 'order-return not-found mapping'],
  ['OrderError::OrderChangeNotFound(_)', 'order-change not-found mapping'],
  ['OrderError::InvalidTransition { .. }', 'order transition mapping'],
  ['OrderError::Database(_)', 'order database mapping'],
  ['OrderError::Core(_)', 'order fallback mapping'],
  ['"ORDER_REQUEST_INVALID"', 'order validation code'],
  ['"ORDER_RESOURCE_NOT_FOUND"', 'order not-found code'],
  ['"ORDER_STATE_CONFLICT"', 'order conflict code'],
  ['"ORDER_TEMPORARILY_UNAVAILABLE"', 'order availability code'],
  ['"ORDER_OPERATION_FAILED"', 'order fallback code'],
  ['fn shipping_profile_graphql_error(', 'shipping-profile mapper'],
  ['CommerceError::Validation(_)', 'shipping validation mapping'],
  ['CommerceError::ShippingProfileNotFound(_)', 'shipping not-found mapping'],
  ['CommerceError::DuplicateShippingProfileSlug(_)', 'shipping conflict mapping'],
  ['CommerceError::Database(_)', 'shipping database mapping'],
  ['CommerceError::Rich(_)', 'shipping rich fallback mapping'],
  ['CommerceError::Core(_)', 'shipping core fallback mapping'],
  ['"SHIPPING_PROFILE_REQUEST_INVALID"', 'shipping validation code'],
  ['"SHIPPING_PROFILE_NOT_FOUND"', 'shipping not-found code'],
  ['"SHIPPING_PROFILE_STATE_CONFLICT"', 'shipping conflict code'],
  ['"SHIPPING_PROFILE_TEMPORARILY_UNAVAILABLE"', 'shipping availability code'],
  ['"SHIPPING_PROFILE_OPERATION_FAILED"', 'shipping fallback code'],
  ['super::cart_safe_helpers::resolve_optional_storefront_customer_id(', 'safe customer lookup reuse'],
  ['pub(crate) async fn ensure_storefront_order_access(', 'safe order-access helper'],
  ['pub(crate) async fn validate_product_shipping_profile_input(', 'safe product shipping helper'],
  ['pub(crate) async fn validate_shipping_option_profile_inputs(', 'safe option shipping helper'],
  ['tenant_id = %tenant_id', 'tenant logging'],
  ['order_id = %order_id', 'order logging'],
  ['operation,', 'operation logging'],
  ['extensions.set("retryable", retryable)', 'retryability extension'],
]) {
  requireText(facadeSource, value, label);
}

const orderMapperCalls = facadeSource.match(/order_graphql_error\(/g) ?? [];
if (orderMapperCalls.length !== 2) {
  failures.push(`expected order mapper definition plus one call, found ${orderMapperCalls.length}`);
}

const shippingMapperCalls = facadeSource.match(/shipping_profile_graphql_error\(/g) ?? [];
if (shippingMapperCalls.length !== 3) {
  failures.push(`expected shipping mapper definition plus two calls, found ${shippingMapperCalls.length}`);
}

if (failures.length > 0) {
  console.error('Commerce GraphQL order and shipping helper error safety verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Commerce GraphQL order access and shipping-profile helpers expose stable public envelopes with internal structured logs',
);

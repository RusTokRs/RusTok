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
const facadeSource = read('crates/rustok-commerce/src/graphql/mutations/safe_helpers.rs');
const failures = [];

const requireText = (source, value, label) => {
  if (!source.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (source, value, label) => {
  if (source.includes(value)) failures.push(`${label}: forbidden ${value}`);
};

for (const [value, label] of [
  ['#[path = "helpers.rs"]\nmod legacy_helpers;', 'private legacy helper routing'],
  ['#[path = "safe_helpers.rs"]\nmod cart_safe_helpers;', 'private cart safe helper routing'],
  ['#[path = "safe_order_helpers.rs"]\npub mod helpers;', 'public layered safe helper routing'],
]) {
  requireText(moduleSource, value, label);
}

for (const value of [
  'async_graphql::Error::new(error.message)',
  'async_graphql::Error::new(error.to_string())',
  'async_graphql::Error::new(format!("{error}"))',
  'pub(crate) use super::legacy_helpers::*',
]) {
  forbidText(facadeSource, value, 'storefront cart safe helper facade');
}

for (const [value, label] of [
  ['PortError, PortErrorKind', 'port error imports'],
  ['fn customer_port_graphql_error(', 'customer mapper'],
  ['pub(crate) fn cart_port_error(', 'cart mapper'],
  ['correlation_id = %context.correlation_id', 'customer correlation logging'],
  ['tenant_id = %context.tenant_id', 'customer tenant logging'],
  ['owner_code = %error.code', 'owner code logging'],
  ['owner_kind = ?error.kind', 'owner kind logging'],
  ['owner_retryable = error.retryable', 'owner retryability logging'],
  ['PortErrorKind::Validation', 'validation mapping'],
  ['PortErrorKind::NotFound', 'not-found mapping'],
  ['PortErrorKind::Conflict', 'conflict mapping'],
  ['PortErrorKind::Forbidden', 'forbidden mapping'],
  ['PortErrorKind::Unavailable | PortErrorKind::Timeout', 'availability mapping'],
  ['PortErrorKind::InvariantViolation', 'invariant mapping'],
  ['"CART_REQUEST_INVALID"', 'cart validation code'],
  ['"CART_RESOURCE_NOT_FOUND"', 'cart not-found code'],
  ['"CART_STATE_CONFLICT"', 'cart conflict code'],
  ['"CART_ACCESS_DENIED"', 'cart forbidden code'],
  ['"CART_TEMPORARILY_UNAVAILABLE"', 'cart availability code'],
  ['"CART_OPERATION_FAILED"', 'cart invariant code'],
  ['"CUSTOMER_TEMPORARILY_UNAVAILABLE"', 'customer availability code'],
  ['fn legacy_graphql_error(', 'legacy envelope interceptor'],
  ['resource_id = ?resource_id', 'resource logging'],
  ['"Cart shipping details are temporarily unavailable"', 'cart enrichment message'],
  ['"Selected shipping option is invalid"', 'shipping selection message'],
  ['"Product is not available"', 'product availability message'],
  ['"Requested quantity is not available"', 'inventory insufficiency message'],
  ['"Cart line item could not be resolved"', 'line item fallback message'],
  ['"Cart pricing could not be refreshed"', 'reprice fallback message'],
  ['"Inventory availability could not be verified"', 'inventory dependency message'],
  ['extensions.set("retryable", retryable)', 'retryability extension'],
]) {
  requireText(facadeSource, value, label);
}

for (const operation of [
  'resolve_optional_storefront_customer_id',
  'enrich_storefront_cart',
  'validate_selected_shipping_option',
  'resolve_storefront_line_item_input',
  'reprice_storefront_cart_line_items',
  'validate_storefront_line_item_quantity',
  'validate_storefront_variant_inventory',
]) {
  requireText(facadeSource, `"${operation}"`, `${operation} operation mapping`);
}

const legacyCalls =
  facadeSource.match(/super::legacy_helpers::[a-z_]+\(/g) ?? [];
if (legacyCalls.length !== 6) {
  failures.push(`expected 6 intercepted legacy helper calls, found ${legacyCalls.length}`);
}

if (failures.length > 0) {
  console.error('Commerce GraphQL storefront cart helper error safety verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Commerce GraphQL storefront cart helpers remain behind stable public envelopes while layered helper routing stays private',
);

#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const source = readFileSync(
  new URL('crates/rustok-commerce/src/graphql/mutations/pricing.rs', root),
  'utf8',
);
const failures = [];

const requireText = (value, label) => {
  if (!source.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (value, label) => {
  if (source.includes(value)) failures.push(`${label}: forbidden ${value}`);
};

for (const value of [
  'async_graphql::Error::new(error.message)',
  '.map_err(cart_port_error)',
  'async_graphql::Error::new(error.to_string())',
  'async_graphql::Error::new(format!("{error}"))',
]) {
  forbidText(value, 'commerce GraphQL pricing public error mapping');
}

for (const [value, label] of [
  ['PortError, PortErrorKind', 'port error imports'],
  ['fn pricing_port_graphql_error(', 'shared pricing GraphQL mapper'],
  ['correlation_id = %context.correlation_id', 'correlation logging'],
  ['tenant_id = %context.tenant_id', 'tenant logging'],
  ['operation,', 'operation logging'],
  ['owner_code = %error.code', 'owner code logging'],
  ['owner_kind = ?error.kind', 'owner kind logging'],
  ['owner_retryable = error.retryable', 'owner retryability logging'],
  ['PortErrorKind::Validation', 'validation mapping'],
  ['PortErrorKind::NotFound', 'not-found mapping'],
  ['PortErrorKind::Conflict', 'conflict mapping'],
  ['PortErrorKind::Forbidden', 'forbidden mapping'],
  ['PortErrorKind::Unavailable | PortErrorKind::Timeout', 'availability mapping'],
  ['PortErrorKind::InvariantViolation', 'invariant mapping'],
  ['"PRICING_REQUEST_INVALID"', 'validation code'],
  ['"PRICING_RESOURCE_NOT_FOUND"', 'not-found code'],
  ['"PRICING_STATE_CONFLICT"', 'conflict code'],
  ['"PRICING_ACCESS_DENIED"', 'forbidden code'],
  ['"PRICING_TEMPORARILY_UNAVAILABLE"', 'availability code'],
  ['"PRICING_OPERATION_FAILED"', 'fallback code'],
  ['"Pricing request is invalid"', 'validation message'],
  ['"Pricing resource was not found"', 'not-found message'],
  ['"Pricing operation conflicts with the current state"', 'conflict message'],
  ['"Pricing operation is not permitted"', 'forbidden message'],
  ['"Pricing is temporarily unavailable"', 'availability message'],
  ['"Pricing operation could not be completed safely"', 'fallback message'],
  ['extensions.set("retryable", retryable)', 'retryability extension'],
]) {
  requireText(value, label);
}

for (const operation of [
  'preview_admin_cart_promotion',
  'apply_admin_cart_promotion',
  'upsert_variant_price',
  'preview_variant_discount',
  'apply_variant_discount',
  'set_price_list_percentage_rule',
  'set_price_list_scope',
]) {
  requireText(`"${operation}"`, `${operation} mapping`);
}

const contextCopies = source.match(/let error_context = port_context\.clone\(\);/g) ?? [];
if (contextCopies.length !== 7) {
  failures.push(`expected 7 preserved error contexts, found ${contextCopies.length}`);
}

const mapperCalls = source.match(/pricing_port_graphql_error\(/g) ?? [];
if (mapperCalls.length !== 8) {
  failures.push(`expected mapper definition plus 7 calls, found ${mapperCalls.length}`);
}

if (failures.length > 0) {
  console.error('Commerce GraphQL pricing error safety verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Commerce GraphQL pricing and cart-promotion port failures expose stable public envelopes with correlation-aware internal logs',
);

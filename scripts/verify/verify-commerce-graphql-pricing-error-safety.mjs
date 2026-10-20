#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const source = readFileSync(
  new URL('crates/modules/rustok-commerce/src/graphql/mutations/pricing.rs', root),
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
  ['fn pricing_port_graphql_error(', 'shared pricing GraphQL mapper'],
  ['rustok_pricing::map_pricing_port_error(&error)', 'Pricing-owned public policy'],
  ['correlation_id = %public.correlation_id', 'opaque public reference logging'],
  ['tenant_id_length = context.tenant_id.chars().count()', 'bounded tenant facts'],
  ['request_correlation_present = !context.correlation_id.is_empty()', 'bounded context facts'],
  ['operation,', 'operation logging'],
  ['owner_code_length = error.code.chars().count()', 'bounded code facts'],
  ['owner_message_length = error.message.chars().count()', 'bounded message facts'],
  ['owner_kind = ?error.kind', 'typed owner kind'],
  ['owner_retryable = error.retryable', 'owner retryability'],
  ['async_graphql::Error::new(public.message)', 'static owner public copy'],
  ['extensions.set("code", public.code)', 'owner public code'],
  ['extensions.set("retryable", public.retryable)', 'owner retryability extension'],
  ['extensions.set("correlation_id", public.correlation_id.to_string())', 'opaque reference extension'],
]) requireText(value, label);
for (const value of [
  'error = ?error', 'correlation_id = %context.correlation_id',
  'tenant_id = %context.tenant_id', 'owner_code = %error.code',
  'error.message.clone()', 'match &error.kind',
]) forbidText(value, 'raw payload or parallel classification');

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

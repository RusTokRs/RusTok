#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const source = readFileSync(
  new URL('crates/rustok-commerce/src/graphql/mutations/safe_helpers.rs', root),
  'utf8',
);
const failures = [];

const requireText = (content, value, label) => {
  if (!content.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (content, value, label) => {
  if (content.includes(value)) failures.push(`${label}: forbidden ${value}`);
};
const between = (content, start, end, label) => {
  const startIndex = content.indexOf(start);
  const endIndex = content.indexOf(end, startIndex + start.length);
  if (startIndex < 0 || endIndex < 0) {
    failures.push(`${label}: unable to isolate source block`);
    return '';
  }
  return content.slice(startIndex, endIndex);
};

const diagnosticType = between(
  source,
  'struct StorefrontCartPortDiagnosticError {',
  'fn actor_kind_name(',
  'cart diagnostic type',
);
const ownerClassifier = between(
  source,
  'fn cart_port_source_owner(',
  'pub(crate) fn cart_port_error(',
  'cart source-owner classifier',
);
const mapper = between(
  source,
  'pub(crate) fn cart_port_error(',
  'pub(crate) async fn resolve_optional_storefront_customer_id(',
  'cart port GraphQL mapper',
);

for (const [value, label] of [
  ['struct StorefrontCartPortDiagnosticError {', 'diagnostic type'],
  ['code: String,', 'owner code fact'],
  ['kind: PortErrorKind,', 'typed owner kind fact'],
  ['retryable: bool,', 'owner retryability fact'],
  ["message_shape: &'static str,", 'owner message shape fact'],
  ['message_len: usize,', 'owner message length fact'],
  ['impl std::fmt::Debug for StorefrontCartPortDiagnosticError', 'custom diagnostic Debug'],
  ['formatter.write_str("redacted")', 'redacted diagnostic Debug output'],
]) requireText(diagnosticType, value, label);
for (const value of ['#[derive(Debug)]', 'formatter.debug_struct(', '.field(']) {
  forbidText(diagnosticType, value, 'cart diagnostic payload exposure');
}

for (const [value, label] of [
  ['Some(("cart", _)) => "rustok_cart"', 'cart owner classification'],
  ['Some(("pricing", _)) => "rustok_pricing"', 'pricing owner classification'],
  ['_ => "unknown"', 'unknown owner classification'],
]) requireText(ownerClassifier, value, label);

for (const [value, label] of [
  ['error: PortError', 'original owner error input'],
  ['PortErrorKind::Validation', 'validation policy'],
  ['PortErrorKind::NotFound', 'not-found policy'],
  ['PortErrorKind::Conflict', 'conflict policy'],
  ['PortErrorKind::Forbidden', 'forbidden policy'],
  ['PortErrorKind::Unavailable | PortErrorKind::Timeout', 'availability policy'],
  ['PortErrorKind::InvariantViolation', 'invariant policy'],
  ['"Cart request is invalid"', 'validation message'],
  ['"CART_REQUEST_INVALID"', 'validation code'],
  ['"Cart resource was not found"', 'not-found message'],
  ['"CART_RESOURCE_NOT_FOUND"', 'not-found code'],
  ['"Cart operation conflicts with the current state"', 'conflict message'],
  ['"CART_STATE_CONFLICT"', 'conflict code'],
  ['"Cart operation is not permitted"', 'forbidden message'],
  ['"CART_ACCESS_DENIED"', 'forbidden code'],
  ['"Cart is temporarily unavailable"', 'availability message'],
  ['"CART_TEMPORARILY_UNAVAILABLE"', 'availability code'],
  ['"Cart operation could not be completed safely"', 'invariant message'],
  ['"CART_OPERATION_FAILED"', 'invariant code'],
  ['let source_owner = cart_port_source_owner(&error);', 'source owner projection'],
  ['let message_shape = text_shape(error.message.as_str());', 'message shape projection'],
  ['let message_len = error.message.len();', 'message length projection'],
  ['let error = StorefrontCartPortDiagnosticError {', 'diagnostic shadow'],
  ['code: error.code,', 'owner code transfer'],
  ['kind: error.kind,', 'owner kind transfer'],
  ['retryable: error.retryable,', 'owner retryability transfer'],
  ['message_shape,', 'message shape transfer'],
  ['message_len,', 'message length transfer'],
  ['tracing::error!(', 'diagnostic event'],
  ['error = ?error', 'redacted error field'],
  ['owner = "rustok_commerce.graphql_cart_helper"', 'boundary owner'],
  ['source_owner,', 'source owner field'],
  ['operation = "storefront_cart_port"', 'operation field'],
  ['owner_code = %error.code', 'owner code field'],
  ['owner_message_shape = error.message_shape', 'message shape field'],
  ['owner_message_len = error.message_len', 'message length field'],
  ['owner_kind = ?error.kind', 'typed owner kind field'],
  ['owner_retryable = error.retryable', 'owner retryability field'],
  ['public_code = code', 'public code field'],
  ['public_retryable = retryable', 'public retryability field'],
  ['boundary = STOREFRONT_CART_HELPER_BOUNDARY', 'shared boundary'],
  [
    '"commerce GraphQL storefront cart or pricing owner port failed"',
    'static event message',
  ],
  ['public_graphql_error(message, code, retryable)', 'stable public envelope'],
]) requireText(mapper, value, label);

for (const value of [
  'internal_message =',
  'owner_message =',
  'message = %error.message',
  'message = ?error.message',
  'error_message =',
  'format!("{error:?}")',
  'async_graphql::Error::new(error.message)',
  'async_graphql::Error::new(error.to_string())',
]) forbidText(mapper, value, 'raw cart port diagnostic or public error');

const policyIndex = mapper.indexOf('let (message, code, retryable) = match &error.kind');
const ownerIndex = mapper.indexOf('let source_owner = cart_port_source_owner(&error);');
const shapeIndex = mapper.indexOf('let message_shape = text_shape(error.message.as_str());');
const shadowIndex = mapper.indexOf('let error = StorefrontCartPortDiagnosticError {');
const eventIndex = mapper.indexOf('tracing::error!(');
const returnIndex = mapper.lastIndexOf('public_graphql_error(message, code, retryable)');
if (
  !(
    policyIndex >= 0 &&
    policyIndex < ownerIndex &&
    ownerIndex < shapeIndex &&
    shapeIndex < shadowIndex &&
    shadowIndex < eventIndex &&
    eventIndex < returnIndex
  )
) {
  failures.push('cart port error must be mapped, projected, shadowed, diagnosed, and returned in order');
}
if ((mapper.match(/let error = StorefrontCartPortDiagnosticError \{/g) ?? []).length !== 1) {
  failures.push('expected one cart port diagnostic shadow');
}
if ((mapper.match(/tracing::error!\(/g) ?? []).length !== 1) {
  failures.push('expected one cart port diagnostic event');
}
if ((mapper.match(/error = \?error/g) ?? []).length !== 1) {
  failures.push('expected one redacted cart port diagnostic error field');
}

if (failures.length > 0) {
  console.error('Commerce GraphQL cart port diagnostic-safety verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Commerce GraphQL cart/pricing port errors keep stable public envelopes and emit bounded redacted diagnostics',
);

#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const source = readFileSync(
  new URL('crates/rustok-cart/src/checkout_snapshot.rs', root),
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
const from = (content, start, label) => {
  const startIndex = content.indexOf(start);
  if (startIndex < 0) {
    failures.push(`${label}: unable to isolate source block`);
    return '';
  }
  return content.slice(startIndex);
};

const portImpl = between(
  source,
  '#[async_trait]\nimpl CartCheckoutPort for InProcessCartCheckoutPort',
  'fn validate_prepare_input(',
  'cart checkout port implementation',
);
const prepare = between(
  portImpl,
  'async fn prepare_checkout(',
  'async fn read_checkout_snapshot(',
  'prepare checkout operation',
);
const readSnapshot = between(
  portImpl,
  'async fn read_checkout_snapshot(',
  'async fn complete_checkout(',
  'read checkout snapshot operation',
);
const complete = between(
  portImpl,
  'async fn complete_checkout(',
  'async fn release_checkout(',
  'complete checkout operation',
);
const release = from(
  portImpl,
  'async fn release_checkout(',
  'release checkout operation',
);
const parser = between(
  source,
  'fn parse_tenant_id(',
  'fn snapshot_from_cart(',
  'cart checkout tenant parser',
);

for (const [block, operation, label] of [
  [prepare, 'PREPARE_CHECKOUT_OPERATION', 'prepare tenant context'],
  [readSnapshot, 'READ_CHECKOUT_SNAPSHOT_OPERATION', 'read tenant context'],
  [complete, 'COMPLETE_CHECKOUT_OPERATION', 'complete tenant context'],
  [release, 'RELEASE_CHECKOUT_OPERATION', 'release tenant context'],
]) {
  const admissionIndex = block.indexOf('require_cart_checkout_');
  const parserCall = `let tenant_id = parse_tenant_id(&context, ${operation})?;`;
  const parserIndex = block.indexOf(parserCall);
  requireText(block, parserCall, label);
  if (!(admissionIndex >= 0 && admissionIndex < parserIndex)) {
    failures.push(`${label}: admission must precede operation-aware tenant parsing`);
  }
}

for (const [value, label] of [
  ['fn parse_tenant_id(', 'tenant parser'],
  ['context: &PortContext', 'retained context input'],
  ["owner_operation: &'static str", 'exact owner operation input'],
  ['Uuid::parse_str(context.tenant_id.as_str()).map_err(|cause| {', 'UUID parse cause capture'],
  ['let error = PortError::validation(', 'stable validation construction'],
  ['"cart.tenant_id_invalid"', 'stable tenant code'],
  ['"PortContext.tenant_id must be a UUID for cart checkout"', 'stable tenant message'],
  ['tracing::warn!(', 'validation warning severity'],
  ['cause = ?cause', 'UUID parse cause evidence'],
  ['error = ?error', 'mapped error evidence'],
  ['owner = CART_CHECKOUT_OWNER', 'truthful owner'],
  ['owner_operation,', 'exact owner operation'],
  ['validation_phase = "tenant_id"', 'tenant validation phase'],
  ['correlation_id = %context.correlation_id', 'correlation context'],
  ['tenant_id = %context.tenant_id', 'tenant context'],
  ['actor = ?context.actor', 'actor context'],
  ['channel = ?context.channel', 'channel context'],
  ['locale = %context.locale', 'locale context'],
  ['causation_id = ?context.causation_id', 'causation context'],
  ['traceparent = ?context.traceparent', 'trace context'],
  ['idempotency_key = ?context.idempotency_key', 'idempotency context'],
  ['deadline_ms = ?context.deadline_ms', 'deadline context'],
  ['internal_code = %error.code', 'stable internal code evidence'],
  ['internal_message = %error.message', 'stable internal message evidence'],
  ['error_kind = ?error.kind', 'validation kind evidence'],
  ['retryable = error.retryable', 'stable retryability evidence'],
  ['boundary = CART_CHECKOUT_BOUNDARY', 'cart checkout boundary'],
  ['"cart checkout owner tenant context was rejected"', 'tenant rejection event'],
  ['error\n    })', 'same error returned'],
]) requireText(parser, value, label);

const warningIndex = parser.indexOf('tracing::warn!(');
const returnIndex = parser.lastIndexOf('error\n    })');
if (!(warningIndex >= 0 && warningIndex < returnIndex)) {
  failures.push('tenant parser: diagnostics must precede returning the stable validation error');
}

for (const value of [
  'fn parse_tenant_id(context: &PortContext)',
  'let tenant_id = parse_tenant_id(&context)?;',
  'Uuid::parse_str(context.tenant_id.as_str()).map_err(|_| {',
]) forbidText(source, value, 'context-dropping tenant parser');

for (const [pattern, expected, label] of [
  [/parse_tenant_id\(/g, 5, 'tenant parser definition/use count'],
  [/validation_phase = "tenant_id"/g, 1, 'tenant validation phase count'],
  [/"cart\.tenant_id_invalid"/g, 1, 'stable tenant code count'],
  [/"PortContext\.tenant_id must be a UUID for cart checkout"/g, 1, 'stable tenant message count'],
  [/"cart checkout owner tenant context was rejected"/g, 1, 'tenant rejection event count'],
]) {
  const count = source.match(pattern)?.length ?? 0;
  if (count !== expected) failures.push(`${label}: expected ${expected}, found ${count}`);
}

for (const [value, label] of [
  ['fn require_cart_checkout_read_admission(', 'read admission helper'],
  ['fn require_cart_checkout_write_admission(', 'write admission helper'],
  ['fn log_cart_checkout_admission_rejection(', 'admission diagnostics'],
  ['fn snapshot_from_cart(', 'snapshot projection'],
  ['fn cart_snapshot_hash(', 'snapshot hash'],
  ['fn projection_hash(', 'projection hash'],
  ['fn normalize_snapshot_value(', 'snapshot normalization'],
  ['fn canonicalize_json(', 'canonical JSON'],
  ['fn merge_checkout_order_metadata(', 'checkout metadata merge'],
  ['fn cart_error_to_port_error(', 'stable public cart mapper'],
]) requireText(source, value, label);

if (failures.length > 0) {
  console.error('Cart checkout tenant context verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Cart checkout tenant validation retains UUID cause, full PortContext, exact owner operation, stable validation envelope, and warning severity',
);

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
  'fn map_cart_checkout_local_port_error(',
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
const mapper = between(
  source,
  'fn map_cart_checkout_local_port_error(',
  'fn map_cart_checkout_service_error(',
  'cart checkout local error mapper',
);

for (const [value, label] of [
  ['let prepare_input_result = (|| {', 'prepare validation result capture'],
  [
    'validate_prepare_input(&request.input).map_err(cart_error_to_port_error)?;',
    'stable prepare validation mapping',
  ],
  ['prepare_input_result.map_err(|error| {', 'prepare validation interception'],
  ['PREPARE_CHECKOUT_OPERATION,\n                "validate_prepare_input",', 'prepare validation context'],
  ['CartStatus::parse(cart.status.as_str()).ok_or_else(|| {', 'prepare status parser'],
  ['PREPARE_CHECKOUT_OPERATION,\n                "parse_cart_status",', 'prepare status context'],
  ['"cart.invalid_status"', 'stable invalid-status code'],
  ['format!("invalid cart status `{}`", cart.status)', 'stable invalid-status message'],
  ['PREPARE_CHECKOUT_OPERATION,\n                    "require_checkout_status",', 'checkout status rejection context'],
  ['"cart.checkout_status_conflict"', 'stable checkout conflict code'],
  [
    'format!("cart cannot enter checkout from `{}`", status.as_str())',
    'stable checkout conflict message',
  ],
  ['snapshot_from_cart(cart).map_err(|error| {', 'prepare snapshot interception'],
  ['PREPARE_CHECKOUT_OPERATION,\n                "snapshot_from_cart",', 'prepare snapshot context'],
]) requireText(prepare, value, label);

for (const [block, operation, label] of [
  [readSnapshot, 'READ_CHECKOUT_SNAPSHOT_OPERATION', 'read snapshot context'],
  [complete, 'COMPLETE_CHECKOUT_OPERATION', 'complete snapshot context'],
  [release, 'RELEASE_CHECKOUT_OPERATION', 'release snapshot context'],
]) {
  requireText(block, '"snapshot_from_cart",', `${label} local operation`);
  requireText(block, operation, `${label} owner operation`);
  requireText(block, 'map_cart_checkout_local_port_error(', `${label} local mapper`);
}
requireText(readSnapshot, '.and_then(snapshot_from_cart);', 'read snapshot projection preserved');
requireText(complete, 'merge_checkout_order_metadata(cart.metadata, request.order_id)', 'completion metadata preserved');

for (const [value, label] of [
  ['fn map_cart_checkout_local_port_error(', 'local mapper definition'],
  ['context: &PortContext', 'retained context input'],
  ["owner_operation: &'static str", 'exact owner operation input'],
  ["local_operation: &'static str", 'exact local operation input'],
  ['error: PortError', 'mapped port error input'],
  [
    'PortErrorKind::Unavailable | PortErrorKind::Timeout | PortErrorKind::InvariantViolation',
    'technical severity classification',
  ],
  ['tracing::error!(', 'technical error severity'],
  ['tracing::warn!(', 'ordinary warning severity'],
  ['error = ?error', 'mapped error evidence'],
  ['owner = "rustok_cart"', 'truthful owner'],
  ['owner_operation,', 'exact owner operation field'],
  ['local_operation,', 'exact local operation field'],
  ['correlation_id = %context.correlation_id', 'correlation context'],
  ['tenant_id = %context.tenant_id', 'tenant context'],
  ['actor = ?context.actor', 'actor context'],
  ['channel = ?context.channel', 'channel context'],
  ['locale = %context.locale', 'locale context'],
  ['causation_id = ?context.causation_id', 'causation context'],
  ['traceparent = ?context.traceparent', 'trace context'],
  ['idempotency_key = ?context.idempotency_key', 'idempotency context'],
  ['deadline_ms = ?context.deadline_ms', 'deadline context'],
  ['internal_code = %error.code', 'stable code evidence'],
  ['internal_message = %error.message', 'stable message evidence'],
  ['error_kind = ?error.kind', 'typed kind evidence'],
  ['retryable = error.retryable', 'retryability evidence'],
  ['boundary = "cart_checkout_port"', 'cart checkout boundary'],
  ['"cart checkout local owner operation failed"', 'technical local event'],
  ['"cart checkout local owner operation was rejected"', 'ordinary local event'],
  ['error\n}', 'same mapped error returned'],
]) requireText(mapper, value, label);

const technicalIndex = mapper.indexOf('tracing::error!(');
const ordinaryIndex = mapper.indexOf('tracing::warn!(');
const returnIndex = mapper.lastIndexOf('error\n}');
if (!(technicalIndex >= 0 && ordinaryIndex >= 0 && technicalIndex < returnIndex && ordinaryIndex < returnIndex)) {
  failures.push('local mapper: diagnostics must precede returning the same PortError');
}

for (const value of [
  'validate_prepare_input(&request.input).map_err(cart_error_to_port_error)?;\n\n        let cart',
  'return Err(PortError::conflict(\n                    "cart.checkout_status_conflict"',
]) forbidText(prepare, value, 'context-dropping local prepare error');

for (const [pattern, expected, label] of [
  [/map_cart_checkout_local_port_error\(/g, 8, 'local mapper definition/use count'],
  [/"validate_prepare_input"/g, 1, 'prepare validation operation count'],
  [/"parse_cart_status"/g, 1, 'status parsing operation count'],
  [/"require_checkout_status"/g, 1, 'status admission operation count'],
  [/"snapshot_from_cart"/g, 4, 'snapshot projection operation count'],
  [/owner = "rustok_cart"/g, 2, 'local owner diagnostic count'],
  [/boundary = "cart_checkout_port"/g, 2, 'local boundary diagnostic count'],
]) {
  const count = source.match(pattern)?.length ?? 0;
  if (count !== expected) failures.push(`${label}: expected ${expected}, found ${count}`);
}

for (const [value, label] of [
  ['fn require_cart_checkout_read_admission(', 'read admission helper'],
  ['fn require_cart_checkout_write_admission(', 'write admission helper'],
  ['fn log_cart_checkout_admission_rejection(', 'admission diagnostics'],
  ['fn map_cart_checkout_service_error(', 'service error diagnostics'],
  ['fn parse_tenant_id(', 'operation-aware tenant parser'],
  ['fn snapshot_from_cart(', 'snapshot projection'],
  ['fn cart_snapshot_hash(', 'snapshot hash'],
  ['fn projection_hash(', 'projection hash'],
  ['fn normalize_snapshot_value(', 'snapshot normalization'],
  ['fn canonicalize_json(', 'canonical JSON'],
  ['fn merge_checkout_order_metadata(', 'checkout metadata merge'],
  ['fn cart_error_to_port_error(', 'stable public cart mapper'],
  ['fn canonical_json_is_independent_of_object_key_order()', 'canonical hash test source'],
  [
    'fn snapshot_normalization_removes_volatile_projection_fields()',
    'snapshot normalization test source',
  ],
]) requireText(source, value, label);

if (failures.length > 0) {
  console.error('Cart checkout local error context verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Cart checkout local validation, lifecycle, and snapshot failures retain full PortContext, exact owner/local operations, stable public PortError values, and technical-versus-ordinary severity',
);

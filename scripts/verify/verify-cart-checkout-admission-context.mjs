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

const admission = between(
  source,
  'fn require_cart_checkout_read_admission(',
  '#[async_trait]\nimpl CartCheckoutPort for InProcessCartCheckoutPort',
  'cart checkout admission helpers',
);
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

for (const [value, label] of [
  ['use rustok_api::{PortCallPolicy, PortContext, PortError, PortErrorKind};', 'typed admission imports'],
  ['const CART_CHECKOUT_OWNER: &str = "rustok_cart";', 'truthful cart owner'],
  ['const CART_CHECKOUT_BOUNDARY: &str = "cart_checkout_port";', 'stable cart checkout boundary'],
  ['const PREPARE_CHECKOUT_OPERATION: &str = "prepare_checkout";', 'prepare operation'],
  [
    'const READ_CHECKOUT_SNAPSHOT_OPERATION: &str = "read_checkout_snapshot";',
    'read snapshot operation',
  ],
  ['const COMPLETE_CHECKOUT_OPERATION: &str = "complete_checkout";', 'complete operation'],
  ['const RELEASE_CHECKOUT_OPERATION: &str = "release_checkout";', 'release operation'],
]) requireText(source, value, label);

for (const [value, label] of [
  ['fn require_cart_checkout_read_admission(', 'read admission helper'],
  ['context.require_policy(PortCallPolicy::read()).map_err(|error| {', 'read policy interception'],
  [
    'log_cart_checkout_admission_rejection(context, owner_operation, "policy", &error);',
    'read policy diagnostics',
  ],
  ['fn require_cart_checkout_write_admission(', 'write admission helper'],
  ['context.require_policy(PortCallPolicy::write()).map_err(|error| {', 'write policy interception'],
  ['context.require_write_semantics().map_err(|error| {', 'write semantics interception'],
  ['"write_semantics",', 'write semantics phase'],
  ['fn log_cart_checkout_admission_rejection(', 'shared rejection diagnostics'],
  ['context: &PortContext', 'retained context input'],
  ["owner_operation: &'static str", 'exact owner operation input'],
  ["admission_phase: &'static str", 'admission phase input'],
  ['error: &PortError', 'original port error input'],
  [
    'PortErrorKind::Unavailable | PortErrorKind::Timeout | PortErrorKind::InvariantViolation',
    'technical severity classification',
  ],
  ['tracing::error!(', 'technical rejection severity'],
  ['tracing::warn!(', 'ordinary rejection severity'],
  ['error = ?error', 'original error evidence'],
  ['owner = CART_CHECKOUT_OWNER', 'truthful owner field'],
  ['owner_operation,', 'exact operation field'],
  ['admission_phase,', 'admission phase field'],
  ['correlation_id = %context.correlation_id', 'correlation context'],
  ['tenant_id = %context.tenant_id', 'tenant context'],
  ['actor = ?context.actor', 'actor context'],
  ['channel = ?context.channel', 'channel context'],
  ['locale = %context.locale', 'locale context'],
  ['causation_id = ?context.causation_id', 'causation context'],
  ['traceparent = ?context.traceparent', 'trace context'],
  ['idempotency_key = ?context.idempotency_key', 'idempotency context'],
  ['deadline_ms = ?context.deadline_ms', 'deadline context'],
  ['internal_code = %error.code', 'original internal code'],
  ['internal_message = %error.message', 'original internal message'],
  ['error_kind = ?error.kind', 'typed error kind'],
  ['retryable = error.retryable', 'original retryability'],
  ['boundary = CART_CHECKOUT_BOUNDARY', 'cart checkout boundary field'],
  ['"cart checkout owner admission failed"', 'technical event'],
  ['"cart checkout owner admission was rejected"', 'ordinary event'],
]) requireText(admission, value, label);

for (const [block, values, label] of [
  [
    prepare,
    [
      'require_cart_checkout_write_admission(&context, PREPARE_CHECKOUT_OPERATION)?;',
      'let tenant_id = parse_tenant_id(&context, PREPARE_CHECKOUT_OPERATION)?;',
      '.get_cart(tenant_id, request.cart_id)',
      '.begin_checkout(tenant_id, request.cart_id)',
      '.update_context(tenant_id, request.cart_id, request.input)',
      'snapshot_from_cart(cart)',
    ],
    'prepare checkout behavior',
  ],
  [
    readSnapshot,
    [
      'require_cart_checkout_read_admission(&context, READ_CHECKOUT_SNAPSHOT_OPERATION)?;',
      'let tenant_id = parse_tenant_id(&context, READ_CHECKOUT_SNAPSHOT_OPERATION)?;',
      '.get_cart(tenant_id, cart_id)',
      '.and_then(snapshot_from_cart)',
    ],
    'read snapshot behavior',
  ],
  [
    complete,
    [
      'require_cart_checkout_write_admission(&context, COMPLETE_CHECKOUT_OPERATION)?;',
      'let tenant_id = parse_tenant_id(&context, COMPLETE_CHECKOUT_OPERATION)?;',
      '.complete_cart(tenant_id, request.cart_id)',
      'merge_checkout_order_metadata(cart.metadata, request.order_id)',
      'snapshot_from_cart(cart)',
    ],
    'complete checkout behavior',
  ],
  [
    release,
    [
      'require_cart_checkout_write_admission(&context, RELEASE_CHECKOUT_OPERATION)?;',
      'let tenant_id = parse_tenant_id(&context, RELEASE_CHECKOUT_OPERATION)?;',
      '.abandon_cart(tenant_id, cart_id)',
      'snapshot_from_cart(cart)',
    ],
    'release checkout behavior',
  ],
]) {
  for (const value of values) requireText(block, value, label);
  const admissionIndex = block.indexOf('require_cart_checkout_');
  const tenantIndex = block.indexOf('let tenant_id = parse_tenant_id(&context,');
  if (!(admissionIndex >= 0 && admissionIndex < tenantIndex)) {
    failures.push(`${label}: admission must precede tenant parsing`);
  }
}

for (const value of [
  'context.require_policy(PortCallPolicy::read())?;',
  'context.require_policy(PortCallPolicy::write())?;',
  'context.require_write_semantics()?;',
]) forbidText(portImpl, value, 'context-dropping direct admission');

for (const [pattern, expected, label] of [
  [/require_cart_checkout_read_admission\(/g, 2, 'read helper definition/use count'],
  [/require_cart_checkout_write_admission\(/g, 4, 'write helper definition/use count'],
  [/log_cart_checkout_admission_rejection\(/g, 4, 'diagnostic helper definition/use count'],
  [/owner = CART_CHECKOUT_OWNER/g, 3, 'owner diagnostic count'],
  [/boundary = CART_CHECKOUT_BOUNDARY/g, 3, 'boundary diagnostic count'],
  [/"policy"/g, 2, 'policy phase count'],
  [/"write_semantics"/g, 1, 'write semantics phase count'],
]) {
  const count = source.match(pattern)?.length ?? 0;
  if (count !== expected) failures.push(`${label}: expected ${expected}, found ${count}`);
}

for (const [value, label] of [
  ['fn snapshot_from_cart(', 'snapshot projection'],
  ['fn cart_snapshot_hash(', 'snapshot hash'],
  ['fn projection_hash(', 'projection hash'],
  ['fn normalize_snapshot_value(', 'snapshot normalization'],
  ['fn canonicalize_json(', 'canonical JSON'],
  ['fn merge_checkout_order_metadata(', 'checkout order metadata merge'],
  ['fn cart_error_to_port_error(', 'stable public cart mapping'],
  ['fn canonical_json_is_independent_of_object_key_order()', 'canonical hash test source'],
  [
    'fn snapshot_normalization_removes_volatile_projection_fields()',
    'snapshot normalization test source',
  ],
]) requireText(source, value, label);

if (failures.length > 0) {
  console.error('Cart checkout admission context verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Cart checkout owner admission retains full PortContext, exact operation and phase, original PortError, stable behavior, and technical-versus-ordinary severity',
);

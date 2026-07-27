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
  'fn map_cart_checkout_service_error(',
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
  'fn map_cart_checkout_service_error(',
  'fn validate_prepare_input(',
  'cart checkout service error mapper',
);
const publicMapper = between(
  source,
  'fn cart_error_to_port_error(',
  '#[cfg(test)]',
  'stable cart public mapper',
);

for (const [block, values, label] of [
  [
    prepare,
    [
      '.get_cart(tenant_id, request.cart_id)',
      'PREPARE_CHECKOUT_OPERATION,\n                    "get_cart",',
      '.begin_checkout(tenant_id, request.cart_id)',
      'PREPARE_CHECKOUT_OPERATION,\n                            "begin_checkout",',
      '.update_context(tenant_id, request.cart_id, request.input)',
      'PREPARE_CHECKOUT_OPERATION,\n                    "update_context",',
    ],
    'prepare service mappings',
  ],
  [
    readSnapshot,
    [
      '.get_cart(tenant_id, cart_id)',
      'READ_CHECKOUT_SNAPSHOT_OPERATION,\n                    "get_cart",',
      '.and_then(snapshot_from_cart)',
    ],
    'read service mapping',
  ],
  [
    complete,
    [
      '.complete_cart(tenant_id, request.cart_id)',
      'COMPLETE_CHECKOUT_OPERATION,\n                    "complete_cart",',
      'merge_checkout_order_metadata(cart.metadata, request.order_id)',
    ],
    'complete service mapping',
  ],
  [
    release,
    [
      '.abandon_cart(tenant_id, cart_id)',
      'RELEASE_CHECKOUT_OPERATION,\n                    "abandon_cart",',
      'snapshot_from_cart(cart)',
    ],
    'release service mapping',
  ],
]) {
  for (const value of values) requireText(block, value, label);
}

for (const value of [
  '.get_cart(tenant_id, request.cart_id)\n            .await\n            .map_err(cart_error_to_port_error)',
  '.begin_checkout(tenant_id, request.cart_id)\n                    .await\n                    .map_err(cart_error_to_port_error)',
  '.update_context(tenant_id, request.cart_id, request.input)\n            .await\n            .map_err(cart_error_to_port_error)',
  '.get_cart(tenant_id, cart_id)\n            .await\n            .map_err(cart_error_to_port_error)',
  '.complete_cart(tenant_id, request.cart_id)\n            .await\n            .map_err(cart_error_to_port_error)',
  '.abandon_cart(tenant_id, cart_id)\n            .await\n            .map_err(cart_error_to_port_error)',
]) forbidText(portImpl, value, 'context-dropping cart service mapping');

for (const [value, label] of [
  ['fn map_cart_checkout_service_error(', 'service error mapper'],
  ['context: &PortContext', 'retained context input'],
  ["owner_operation: &'static str", 'exact owner operation input'],
  ["service_operation: &'static str", 'exact service operation input'],
  ['error: CartError', 'original typed cart error input'],
  [
    'let (public_code, public_retryable, technical) = match &error {',
    'public classification before diagnostics',
  ],
  ['CartError::Validation(_) => ("cart.checkout_validation", false, false)', 'validation public outcome'],
  ['CartError::CartNotFound(_) => ("cart.not_found", false, false)', 'cart not-found public outcome'],
  [
    'CartError::CartLineItemNotFound(_) => ("cart.line_item_not_found", false, false)',
    'line-item not-found public outcome',
  ],
  [
    'CartError::InvalidTransition { .. } => ("cart.checkout_status_conflict", false, false)',
    'transition public outcome',
  ],
  ['CartError::Database(_) => ("cart.database_unavailable", true, true)', 'database public outcome'],
  ['CartError::TaxBoundary {', 'tax-boundary public outcome'],
  ['code.as_str()', 'tax-boundary public code'],
  ['*retryable', 'tax-boundary public retryability'],
  [
    'PortErrorKind::Unavailable\n                    | PortErrorKind::Timeout\n                    | PortErrorKind::InvariantViolation',
    'technical tax-boundary classification',
  ],
  ['if technical {', 'technical severity selection'],
  ['tracing::error!(', 'technical error severity'],
  ['tracing::warn!(', 'ordinary warning severity'],
  ['error = ?error', 'original typed error evidence'],
  ['owner = CART_CHECKOUT_OWNER', 'truthful owner field'],
  ['owner_operation,', 'exact owner operation field'],
  ['service_operation,', 'exact service operation field'],
  ['correlation_id = %context.correlation_id', 'correlation context'],
  ['tenant_id = %context.tenant_id', 'tenant context'],
  ['actor = ?context.actor', 'actor context'],
  ['channel = ?context.channel', 'channel context'],
  ['locale = %context.locale', 'locale context'],
  ['causation_id = ?context.causation_id', 'causation context'],
  ['traceparent = ?context.traceparent', 'trace context'],
  ['idempotency_key = ?context.idempotency_key', 'idempotency context'],
  ['deadline_ms = ?context.deadline_ms', 'deadline context'],
  ['public_code,', 'selected public code field'],
  ['public_retryable,', 'selected public retryability field'],
  ['boundary = CART_CHECKOUT_BOUNDARY', 'cart checkout boundary field'],
  ['"cart checkout owner service operation failed"', 'technical event'],
  ['"cart checkout owner service operation was rejected"', 'ordinary event'],
  ['cart_error_to_port_error(error)', 'unchanged public mapper delegation'],
]) requireText(mapper, value, label);

const technicalLogIndex = mapper.indexOf('tracing::error!(');
const ordinaryLogIndex = mapper.indexOf('tracing::warn!(');
const publicMapIndex = mapper.lastIndexOf('cart_error_to_port_error(error)');
if (
  !(
    technicalLogIndex >= 0 &&
    ordinaryLogIndex >= 0 &&
    technicalLogIndex < publicMapIndex &&
    ordinaryLogIndex < publicMapIndex
  )
) {
  failures.push('service mapper: diagnostics must precede unchanged public mapping');
}

for (const [pattern, expected, label] of [
  [/map_cart_checkout_service_error\(/g, 7, 'service mapper definition/use count'],
  [/"get_cart"/g, 2, 'get-cart service operation count'],
  [/"begin_checkout"/g, 1, 'begin-checkout service operation count'],
  [/"update_context"/g, 1, 'update-context service operation count'],
  [/"complete_cart"/g, 1, 'complete-cart service operation count'],
  [/"abandon_cart"/g, 1, 'abandon-cart service operation count'],
  [/owner = CART_CHECKOUT_OWNER/g, 5, 'owner diagnostic count'],
  [/boundary = CART_CHECKOUT_BOUNDARY/g, 5, 'boundary diagnostic count'],
]) {
  const count = source.match(pattern)?.length ?? 0;
  if (count !== expected) failures.push(`${label}: expected ${expected}, found ${count}`);
}

const directPortMappings = portImpl.match(/\.map_err\(cart_error_to_port_error\)/g)?.length ?? 0;
if (directPortMappings !== 1) {
  failures.push(
    `local prepare-input mapping count: expected 1 direct public mapper use, found ${directPortMappings}`,
  );
}
requireText(
  prepare,
  'validate_prepare_input(&request.input).map_err(cart_error_to_port_error)?;',
  'local prepare-input mapping preserved',
);

for (const [value, label] of [
  ['CartError::Validation(message)', 'validation mapping'],
  ['"cart.checkout_validation"', 'stable validation code'],
  ['"cart checkout request or projection is invalid"', 'stable validation message'],
  ['CartError::CartNotFound(_)', 'cart not-found mapping'],
  ['"cart.not_found"', 'stable cart not-found code'],
  ['"cart was not found"', 'stable cart not-found message'],
  ['CartError::CartLineItemNotFound(_)', 'line-item not-found mapping'],
  ['"cart.line_item_not_found"', 'stable line-item code'],
  ['"cart line item was not found"', 'stable line-item message'],
  ['CartError::InvalidTransition { .. }', 'transition mapping'],
  ['"cart.checkout_status_conflict"', 'stable transition code'],
  ['"cart status transition conflicts with checkout lifecycle"', 'stable transition message'],
  ['CartError::Database(error)', 'database mapping'],
  ['"cart.database_unavailable"', 'stable database code'],
  ['"cart storage is temporarily unavailable"', 'stable database message'],
  ['CartError::TaxBoundary {', 'tax boundary mapping'],
  ['PortError::new(kind, code, message, retryable)', 'tax boundary propagation'],
]) requireText(publicMapper, value, label);

for (const [value, label] of [
  ['fn require_cart_checkout_read_admission(', 'read admission helper'],
  ['fn require_cart_checkout_write_admission(', 'write admission helper'],
  ['fn log_cart_checkout_admission_rejection(', 'admission diagnostics'],
  ['fn parse_tenant_id(', 'operation-aware tenant parser'],
  ['fn snapshot_from_cart(', 'snapshot projection'],
  ['.map_err(cart_error_to_port_error)?;', 'local snapshot mapping'],
  ['fn cart_snapshot_hash(', 'snapshot hash'],
  ['fn projection_hash(', 'projection hash'],
  ['fn normalize_snapshot_value(', 'snapshot normalization'],
  ['fn canonicalize_json(', 'canonical JSON'],
  ['fn merge_checkout_order_metadata(', 'checkout metadata merge'],
  ['fn canonical_json_is_independent_of_object_key_order()', 'canonical hash test source'],
  [
    'fn snapshot_normalization_removes_volatile_projection_fields()',
    'snapshot normalization test source',
  ],
]) requireText(source, value, label);

if (failures.length > 0) {
  console.error('Cart checkout service error context verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Cart checkout service failures retain full PortContext, exact owner and service operations, original CartError, stable public outcomes, and technical-versus-ordinary severity',
);

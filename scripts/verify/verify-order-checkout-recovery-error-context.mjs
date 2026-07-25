#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const read = (relativePath) => readFileSync(new URL(relativePath, root), 'utf8');

const source = read('crates/rustok-order/src/checkout_order_recovery.rs');
const portContract = read('crates/rustok-api/src/ports.rs');
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

const readCheckout = between(
  source,
  'pub async fn read_checkout_order(',
  'async fn resume_order(',
  'checkout order read',
);
const resumeOrder = between(
  source,
  'async fn resume_order(',
  'async fn load_order(',
  'checkout order lifecycle recovery',
);
const identityValidation = between(
  source,
  'fn validate_identity(',
  'fn require_operation_context(',
  'checkout recovery identity validation',
);
const operationContext = between(
  source,
  'fn require_operation_context(',
  'fn parse_tenant_id(',
  'checkout recovery causation validation',
);
const tenantParser = between(
  source,
  'fn parse_tenant_id(',
  'fn parse_actor_id(',
  'checkout recovery tenant parser',
);
const actorParser = between(
  source,
  'fn parse_actor_id(',
  'fn checkout_request_hashes(',
  'checkout recovery actor parser',
);
const requestEncoding = between(
  source,
  'fn checkout_request_hashes(',
  'fn hash_json(',
  'checkout recovery request encoding',
);
const canonicalEncoding = between(
  source,
  'fn hash_json(',
  'fn canonicalize_json(',
  'checkout recovery canonical encoding',
);
const hashValidation = between(
  source,
  'fn normalize_hash(',
  'fn order_error_to_port_error(',
  'checkout recovery hash validation',
);
const ownerMapper = source.slice(source.indexOf('fn order_error_to_port_error('));

for (const [value, label] of [
  [
    'const CHECKOUT_ORDER_RECOVERY_OWNER: &str = "rustok_order.checkout_order_recovery";',
    'checkout recovery owner constant',
  ],
  ['const RECOVER_OPERATION: &str = "recover_existing_checkout";', 'recovery operation'],
  ['const READ_OPERATION: &str = "read_checkout_order";', 'read operation'],
  ['validate_identity(\n            &context,', 'context-aware identity validation'],
  [
    'order_error_to_port_error(\n                            context,\n                            "confirm_recovered_checkout_order"',
    'context-aware recovery confirmation mapping',
  ],
  [
    'order_error_to_port_error(context, "load_checkout_order", error)',
    'context-aware order load mapping',
  ],
]) requireText(source, value, label);

for (const [block, label] of [
  [readCheckout, 'checkout order read'],
  [resumeOrder, 'checkout order lifecycle recovery'],
  [identityValidation, 'checkout recovery identity validation'],
  [operationContext, 'checkout recovery causation validation'],
  [tenantParser, 'checkout recovery tenant parser'],
  [actorParser, 'checkout recovery actor parser'],
  [requestEncoding, 'checkout recovery request encoding'],
  [canonicalEncoding, 'checkout recovery canonical encoding'],
  [hashValidation, 'checkout recovery hash validation'],
]) {
  for (const [value, detail] of [
    ['owner = CHECKOUT_ORDER_RECOVERY_OWNER', `${label} owner log`],
    ['correlation_id = %context.correlation_id', `${label} correlation log`],
    ['tenant_id = %context.tenant_id', `${label} tenant log`],
    ['channel = ?context.channel', `${label} channel log`],
  ]) requireText(block, value, detail);
}

for (const [block, value, label] of [
  [readCheckout, 'code = "order.checkout_order_not_found"', 'read missing-identity stable code'],
  [readCheckout, 'checkout_operation_id = %request.checkout_operation_id', 'read missing operation identity'],
  [resumeOrder, 'code = "order.checkout_order_cancelled"', 'cancelled lifecycle stable code'],
  [resumeOrder, 'order_state = ?OrderStatusKind::Cancelled', 'cancelled lifecycle state'],
  [resumeOrder, 'code = "order.checkout_order_status_invalid"', 'unknown lifecycle stable code'],
  [resumeOrder, 'order_state = ?OrderStatusKind::Unknown', 'unknown lifecycle state'],
  [identityValidation, 'base_matches,', 'identity base-match evidence'],
  [identityValidation, 'owner_hashes_match,', 'identity owner-hash evidence'],
  [identityValidation, 'legacy_hashes_match,', 'identity legacy-hash evidence'],
  [identityValidation, 'identity_order_id = %identity.order_id', 'identity durable order evidence'],
  [operationContext, 'actual_causation_id = ?context.causation_id', 'actual causation evidence'],
  [tenantParser, 'error = ?error', 'tenant parse cause'],
  [actorParser, 'error = ?error', 'actor parse cause'],
  [requestEncoding, 'error = ?error', 'request encoding cause'],
  [canonicalEncoding, 'error = ?error', 'canonical encoding cause'],
  [hashValidation, 'value_length = value.len()', 'hash validation length evidence'],
]) requireText(block, value, label);

for (const [value, label] of [
  ['OrderError::Database(error)', 'database cause capture'],
  ['OrderError::OrderNotFound(order_id)', 'order identity capture'],
  ['order_id = %order_id', 'order identity log'],
  ['OrderError::Validation(cause)', 'validation cause capture'],
  ['cause = %cause', 'validation cause log'],
  ['OrderError::InvalidTransition { from, to }', 'transition cause capture'],
  ['from = %from', 'transition source log'],
  ['to = %to', 'transition target log'],
  ['OrderError::OrderReturnNotFound(return_id)', 'return identity capture'],
  ['resource_id = %return_id', 'return identity log'],
  ['OrderError::OrderChangeNotFound(change_id)', 'change identity capture'],
  ['resource_id = %change_id', 'change identity log'],
  ['OrderError::Core(error)', 'core cause capture'],
]) requireText(ownerMapper, value, label);

for (const value of [
  'owner = CHECKOUT_ORDER_RECOVERY_OWNER',
  'correlation_id = %context.correlation_id',
  'tenant_id = %context.tenant_id',
  'channel = ?context.channel',
  'operation,',
]) requireText(ownerMapper, value, `owner mapper ${value}`);

for (const [value, label] of [
  [
    '"checkout order was not found for the requested operation"',
    'static checkout-order not-found envelope',
  ],
  ['"checkout order is already cancelled"', 'static cancelled envelope'],
  [
    '"checkout order has an unsupported lifecycle state"',
    'static unsupported-state envelope',
  ],
  [
    '"checkout operation is already bound to a different completion request"',
    'static identity-conflict envelope',
  ],
  ['"checkout operation context is invalid"', 'static causation envelope'],
  ['"order request context is invalid"', 'static owner-context envelope'],
  ['"checkout completion request could not be encoded"', 'static encoding envelope'],
  ['"checkout hash evidence is invalid"', 'static hash-validation envelope'],
  ['"order storage is temporarily unavailable"', 'static storage envelope'],
  ['"order was not found"', 'static order not-found envelope'],
  ['"checkout order recovery request is invalid"', 'static owner-validation envelope'],
  [
    '"order lifecycle transition conflicts with checkout recovery"',
    'static transition envelope',
  ],
  ['"related order resource was not found"', 'static related-resource envelope'],
  ['"order operation failed an internal invariant"', 'static invariant envelope'],
]) requireText(source, value, label);

for (const value of [
  'OrderError::OrderNotFound(_)',
  'OrderError::InvalidTransition { .. }',
  'OrderError::OrderReturnNotFound(_) | OrderError::OrderChangeNotFound(_)',
  '.map_err(order_error_to_port_error)',
  'PortError::validation("order.checkout_recovery_validation", cause)',
  'snapshot_hash =',
  'request_hash =',
]) forbidText(source, value, 'unsafe checkout recovery mapping');

for (const [value, label] of [
  ['pub struct PortContext {', 'shared port context'],
  ['pub correlation_id: String', 'shared correlation field'],
  ['pub channel: Option<String>', 'shared channel field'],
  ['pub struct PortError {', 'shared port error'],
  ['pub fn validation(', 'typed validation constructor'],
  ['pub fn conflict(', 'typed conflict constructor'],
  ['pub fn invariant_violation(', 'typed invariant constructor'],
]) requireText(portContract, value, label);

if (failures.length > 0) {
  console.error('Order checkout recovery error-context verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Order checkout recovery retains owner, channel, correlation, reconciliation evidence, and static public envelopes',
);

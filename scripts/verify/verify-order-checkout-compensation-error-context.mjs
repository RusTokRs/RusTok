#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const read = (relativePath) => readFileSync(new URL(relativePath, root), 'utf8');

const source = read('crates/rustok-order/src/checkout_compensation.rs');
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

const cancellation = between(
  source,
  'async fn cancel_or_adopt_cancelled(',
  'pub fn in_process_checkout_order_compensation_port(',
  'checkout cancellation/adoption implementation',
);
const compensation = between(
  source,
  'impl CheckoutOrderCompensationPort for InProcessCheckoutOrderCompensationPort {',
  'fn validate_identity(',
  'checkout compensation implementation',
);
const invalidRequest = between(
  compensation,
  'if request.checkout_operation_id.is_nil() || request.cart_id.is_nil() {',
  'let Some(identity) = self.resolve_identity',
  'checkout compensation request validation',
);
const missingIdentity = between(
  compensation,
  'let Some(identity) = self.resolve_identity',
  'validate_identity(&context, tenant_id, &request, &identity)?;',
  'missing durable checkout identity',
);
const identityValidation = between(
  source,
  'fn validate_identity(',
  'fn require_operation_context(',
  'checkout compensation identity validation',
);
const operationContext = between(
  source,
  'fn require_operation_context(',
  'fn parse_tenant_id(',
  'checkout compensation causation validation',
);
const tenantParser = between(
  source,
  'fn parse_tenant_id(',
  'fn parse_actor_id(',
  'checkout compensation tenant parser',
);
const actorParser = between(
  source,
  'fn parse_actor_id(',
  'fn manual_reconciliation(',
  'checkout compensation actor parser',
);
const reconciliation = between(
  source,
  'fn manual_reconciliation(',
  'fn order_error_to_port_error(',
  'checkout compensation reconciliation mapper',
);
const ownerMapper = source.slice(source.indexOf('fn order_error_to_port_error('));

for (const [value, label] of [
  [
    'const ORDER_COMPENSATION_OWNER: &str = "rustok_order.checkout_compensation";',
    'compensation owner constant',
  ],
  ['const COMPENSATE_OPERATION: &str = "compensate_checkout_order";', 'compensation operation'],
  ['parse_tenant_id(&context, COMPENSATE_OPERATION)', 'context-aware tenant parsing'],
  ['parse_actor_id(&context, COMPENSATE_OPERATION)', 'context-aware actor parsing'],
  [
    'validate_identity(&context, tenant_id, &request, &identity)?;',
    'context-aware identity validation',
  ],
  [
    'order_error_to_port_error(&context, "read_checkout_order_for_compensation", error)',
    'context-aware order read mapping',
  ],
]) requireText(source, value, label);

for (const [block, label] of [
  [invalidRequest, 'checkout compensation request validation'],
  [identityValidation, 'checkout compensation identity validation'],
  [operationContext, 'checkout compensation causation validation'],
  [tenantParser, 'checkout compensation tenant parser'],
  [actorParser, 'checkout compensation actor parser'],
  [reconciliation, 'checkout compensation reconciliation mapper'],
]) {
  for (const [value, detail] of [
    ['owner = ORDER_COMPENSATION_OWNER', `${label} owner log`],
    ['correlation_id = %context.correlation_id', `${label} correlation log`],
    ['tenant_id = %context.tenant_id', `${label} tenant log`],
    ['channel = ?context.channel', `${label} channel log`],
  ]) requireText(block, value, detail);
}

for (const [value, label] of [
  ['Err(OrderError::InvalidTransition { from, to })', 'transition race cause capture'],
  ['current_state = ?current.status_kind()', 'transition race current state'],
  ['from = %from', 'transition race source state'],
  ['to = %to', 'transition race target state'],
  ['state @ (OrderStatusKind::Paid', 'financial-effect typed lifecycle reconciliation'],
  ['Some(order.id)', 'known order identity reconciliation'],
  ['request.expected_order_id', 'missing identity reconciliation evidence'],
]) requireText(cancellation + missingIdentity, value, label);

for (const [block, value, label] of [
  [invalidRequest, 'expected_order_id = ?request.expected_order_id', 'request expected order evidence'],
  [identityValidation, 'code = "order.checkout_compensation_identity_conflict"', 'identity conflict stable code'],
  [identityValidation, 'identity_order_id = %identity.order_id', 'durable order identity evidence'],
  [identityValidation, 'identity_source_cart_id = ?identity.source_cart_id', 'durable cart identity evidence'],
  [operationContext, 'actual_causation_id = ?context.causation_id', 'actual causation evidence'],
  [tenantParser, 'error = ?error', 'tenant parse cause'],
  [actorParser, 'error = ?error', 'actor parse cause'],
  [reconciliation, 'order_id: Option<Uuid>', 'truthful optional order identity'],
  [reconciliation, 'order_state = ?order_state', 'typed reconciliation state'],
  [reconciliation, 'reason = %reason', 'internal reconciliation cause'],
]) requireText(block, value, label);

for (const [value, label] of [
  ['OrderError::OrderNotFound(order_id)', 'order not-found identity capture'],
  ['order_id = %order_id', 'order not-found identity log'],
  ['OrderError::Validation(cause)', 'validation cause capture'],
  ['cause = %cause', 'validation cause log'],
  ['OrderError::InvalidTransition { from, to }', 'owner transition cause capture'],
  ['from = %from', 'owner transition source log'],
  ['to = %to', 'owner transition target log'],
  ['OrderError::OrderReturnNotFound(return_id)', 'return identity capture'],
  ['resource_id = %return_id', 'return identity log'],
  ['OrderError::OrderChangeNotFound(change_id)', 'change identity capture'],
  ['resource_id = %change_id', 'change identity log'],
  ['OrderError::Database(error)', 'database cause capture'],
  ['OrderError::Core(error)', 'core cause capture'],
]) requireText(ownerMapper, value, label);

for (const value of [
  'owner = ORDER_COMPENSATION_OWNER',
  'correlation_id = %context.correlation_id',
  'tenant_id = %context.tenant_id',
  'channel = ?context.channel',
  'operation,',
]) requireText(ownerMapper, value, `owner mapper ${value}`);

for (const [value, label] of [
  ['"checkout compensation request is invalid"', 'static request-validation envelope'],
  [
    '"checkout order identity conflicts with the compensation request"',
    'static identity-conflict envelope',
  ],
  ['"checkout operation context is invalid"', 'static causation envelope'],
  ['"order request context is invalid"', 'static owner-context envelope'],
  ['"checkout requires manual reconciliation"', 'static reconciliation envelope'],
  ['"order storage is temporarily unavailable"', 'static storage envelope'],
  ['"order was not found"', 'static order not-found envelope'],
  ['"checkout order compensation request is invalid"', 'static owner-validation envelope'],
  ['"checkout order lifecycle conflicts with compensation"', 'static owner-transition envelope'],
  ['"related order resource was not found"', 'static related-resource envelope'],
  ['"order compensation failed an internal invariant"', 'static invariant envelope'],
]) requireText(source, value, label);

for (const value of [
  'validate_identity(tenant_id, &request, &identity)',
  'OrderError::OrderNotFound(_)',
  'OrderError::InvalidTransition { .. }',
  'OrderError::OrderReturnNotFound(_) | OrderError::OrderChangeNotFound(_)',
  '.map_err(order_error_to_port_error)',
  'PortError::validation("order.checkout_compensation_validation", cause)',
  'unwrap_or_default()',
]) forbidText(source, value, 'unsafe checkout compensation mapping');

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
  console.error('Order checkout compensation error-context verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Order checkout compensation retains owner, channel, correlation, reconciliation evidence, and static public envelopes',
);

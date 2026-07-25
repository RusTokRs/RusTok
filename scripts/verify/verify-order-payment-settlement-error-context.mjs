#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const read = (relativePath) => readFileSync(new URL(relativePath, root), 'utf8');

const source = read('crates/rustok-order/src/checkout_payment_settlement.rs');
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

const settlement = between(
  source,
  'impl CheckoutOrderPaymentSettlementPort for InProcessCheckoutOrderPaymentSettlementPort {',
  'fn validate_request(',
  'checkout payment settlement implementation',
);
const identityMissing = between(
  settlement,
  'let identity = identity.ok_or_else(|| {',
  'if identity.tenant_id != tenant_id',
  'missing checkout order identity',
);
const identityConflict = between(
  settlement,
  'if identity.tenant_id != tenant_id',
  'let current = self.load_order',
  'checkout order identity conflict',
);
const lifecycleConflict = between(
  settlement,
  'state @ (OrderStatusKind::Pending',
  'if settled.payment_id.as_deref()',
  'checkout payment lifecycle conflict',
);
const paymentReferenceConflict = between(
  settlement,
  'if settled.payment_id.as_deref()',
  'Ok(settled)',
  'checkout payment reference conflict',
);
const requestValidation = between(
  source,
  'fn validate_request(',
  'fn require_operation_context(',
  'checkout payment request validation',
);
const operationContext = between(
  source,
  'fn require_operation_context(',
  'fn parse_tenant_id(',
  'checkout payment causation validation',
);
const tenantParser = between(
  source,
  'fn parse_tenant_id(',
  'fn parse_actor_id(',
  'checkout payment tenant parser',
);
const actorParser = between(
  source,
  'fn parse_actor_id(',
  'fn order_error_to_port_error(',
  'checkout payment actor parser',
);
const ownerMapper = source.slice(source.indexOf('fn order_error_to_port_error('));

for (const [value, label] of [
  [
    'const ORDER_PAYMENT_SETTLEMENT_OWNER: &str = "rustok_order.checkout_payment_settlement";',
    'settlement owner constant',
  ],
  ['const SETTLE_PAYMENT_OPERATION: &str = "settle_checkout_payment";', 'settlement operation'],
  ['parse_tenant_id(&context, SETTLE_PAYMENT_OPERATION)', 'context-aware tenant parsing'],
  ['parse_actor_id(&context, SETTLE_PAYMENT_OPERATION)', 'context-aware actor parsing'],
  ['validate_request(&context, &request)', 'context-aware request validation'],
  [
    'order_error_to_port_error(&context, "mark_checkout_order_paid", error)',
    'context-aware mark-paid mapping',
  ],
]) requireText(source, value, label);

for (const [block, label] of [
  [identityMissing, 'missing checkout order identity'],
  [identityConflict, 'checkout order identity conflict'],
  [lifecycleConflict, 'checkout payment lifecycle conflict'],
  [paymentReferenceConflict, 'checkout payment reference conflict'],
  [requestValidation, 'checkout payment request validation'],
  [operationContext, 'checkout payment causation validation'],
  [tenantParser, 'checkout payment tenant parser'],
  [actorParser, 'checkout payment actor parser'],
]) {
  for (const [value, detail] of [
    ['owner = ORDER_PAYMENT_SETTLEMENT_OWNER', `${label} owner log`],
    ['correlation_id = %context.correlation_id', `${label} correlation log`],
    ['tenant_id = %context.tenant_id', `${label} tenant log`],
    ['channel = ?context.channel', `${label} channel log`],
  ]) requireText(block, value, detail);
}

for (const [block, value, label] of [
  [identityMissing, 'code = "order.checkout_payment_identity_missing"', 'identity-missing stable code'],
  [identityMissing, 'checkout_operation_id = %request.checkout_operation_id', 'identity-missing operation identity'],
  [identityConflict, 'code = "order.checkout_payment_identity_conflict"', 'identity-conflict stable code'],
  [identityConflict, 'identity_order_id = %identity.order_id', 'identity-conflict durable order identity'],
  [identityConflict, 'identity_payment_collection_id = ?identity.payment_collection_id', 'identity-conflict payment identity'],
  [lifecycleConflict, 'code = "order.checkout_payment_state_conflict"', 'lifecycle stable code'],
  [lifecycleConflict, 'order_state = ?state', 'lifecycle internal state'],
  [paymentReferenceConflict, 'code = "order.checkout_payment_reference_conflict"', 'payment-reference stable code'],
  [paymentReferenceConflict, 'settled_payment_reference = ?settled.payment_id', 'settled payment reference'],
  [requestValidation, 'payment_reference_present = !request.payment_reference.trim().is_empty()', 'request reference presence'],
  [operationContext, 'actual_causation_id = ?context.causation_id', 'actual causation evidence'],
  [tenantParser, 'error = ?error', 'tenant parse cause'],
  [actorParser, 'error = ?error', 'actor parse cause'],
]) requireText(block, value, label);

for (const [value, label] of [
  ['OrderError::OrderNotFound(order_id)', 'order not-found identity capture'],
  ['order_id = %order_id', 'order not-found identity log'],
  ['OrderError::Validation(cause)', 'validation cause capture'],
  ['cause = %cause', 'validation cause log'],
  ['OrderError::InvalidTransition { from, to }', 'transition cause capture'],
  ['from = %from', 'transition source log'],
  ['to = %to', 'transition target log'],
  ['OrderError::OrderReturnNotFound(return_id)', 'return identity capture'],
  ['resource_id = %return_id', 'return identity log'],
  ['OrderError::OrderChangeNotFound(change_id)', 'change identity capture'],
  ['resource_id = %change_id', 'change identity log'],
  ['OrderError::Database(error)', 'database cause capture'],
  ['OrderError::Core(error)', 'core cause capture'],
]) requireText(ownerMapper, value, label);

for (const value of [
  'owner = ORDER_PAYMENT_SETTLEMENT_OWNER',
  'correlation_id = %context.correlation_id',
  'tenant_id = %context.tenant_id',
  'channel = ?context.channel',
  'operation,',
]) requireText(ownerMapper, value, `owner mapper ${value}`);

for (const [value, label] of [
  ['"checkout requires manual reconciliation"', 'static identity-missing envelope'],
  [
    '"checkout order identity conflicts with the payment settlement request"',
    'static identity-conflict envelope',
  ],
  [
    '"checkout order lifecycle does not allow payment settlement"',
    'static lifecycle-conflict envelope',
  ],
  [
    '"checkout order is settled by another payment identity"',
    'static payment-reference envelope',
  ],
  ['"checkout payment settlement request is invalid"', 'static request-validation envelope'],
  ['"checkout operation context is invalid"', 'static causation envelope'],
  ['"order request context is invalid"', 'static owner-context envelope'],
  ['"order storage is temporarily unavailable"', 'static storage envelope'],
  ['"order was not found"', 'static order not-found envelope'],
  [
    '"checkout order payment settlement request is invalid"',
    'static owner-validation envelope',
  ],
  ['"order lifecycle conflicts with payment settlement"', 'static owner-transition envelope'],
  ['"related order resource was not found"', 'static related-resource envelope'],
  [
    '"order payment settlement failed an internal invariant"',
    'static invariant envelope',
  ],
]) requireText(source, value, label);

for (const value of [
  'OrderError::OrderNotFound(_)',
  'OrderError::InvalidTransition { .. }',
  'OrderError::OrderReturnNotFound(_) | OrderError::OrderChangeNotFound(_)',
  '.map_err(order_error_to_port_error)',
  'PortError::validation("order.checkout_payment_validation", cause)',
]) forbidText(source, value, 'unsafe checkout payment settlement mapping');

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
  console.error('Order payment settlement error-context verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Order payment settlement retains owner, channel, correlation, reconciliation evidence, and static public envelopes',
);

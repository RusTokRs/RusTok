#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const read = (relativePath) => readFileSync(new URL(relativePath, root), 'utf8');

const wrapper = read('crates/rustok-order/src/checkout_owner_context.rs');
const settlement = read('crates/rustok-order/src/checkout_payment_settlement.rs');
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

const wrapperImpl = between(
  wrapper,
  'impl CheckoutOrderPaymentSettlementPort for InProcessCheckoutOrderPaymentSettlementPort {',
  'pub fn in_process_checkout_order_payment_settlement_port(',
  'payment settlement wrapper implementation',
);
const mapper = between(
  wrapper,
  'fn map_checkout_order_payment_settlement_local_port_error(',
  'fn require_order_checkout_write_admission(',
  'payment settlement local mapper',
);
const ownerMapper = settlement.slice(settlement.indexOf('fn order_error_to_port_error('));

for (const [value, label] of [
  ['const ORDER_PAYMENT_SETTLEMENT_OWNER: &str = "rustok_order.checkout_payment_settlement";', 'truthful settlement owner'],
  ['const ORDER_PAYMENT_SETTLEMENT_BOUNDARY: &str = "checkout_order_payment_settlement_port";', 'settlement boundary'],
  ['const SETTLE_PAYMENT_OPERATION: &str = "settle_checkout_payment";', 'public settlement operation'],
  ['let diagnostic_context = context.clone();', 'delegated context retention'],
  ['let result = self.inner.settle_checkout_payment(context, request).await;', 'unchanged owner delegation'],
  ['result.map_err(|error| {', 'post-delegation local mapping'],
  ['map_checkout_order_payment_settlement_local_port_error(&diagnostic_context, error)', 'retained context mapper call'],
]) requireText(wrapperImpl, value, label);

const delegationIndex = wrapperImpl.indexOf('self.inner.settle_checkout_payment(context, request).await');
const mapperCallIndex = wrapperImpl.indexOf('map_checkout_order_payment_settlement_local_port_error(');
if (!(delegationIndex >= 0 && delegationIndex < mapperCallIndex)) {
  failures.push('settlement wrapper must delegate before mapping the returned local PortError');
}

for (const [code, message, localOperation, label] of [
  [
    'order.checkout_payment_request_invalid',
    'checkout payment settlement request is invalid',
    'validate_request',
    'request validation outcome',
  ],
  [
    'order.checkout_payment_identity_missing',
    'checkout requires manual reconciliation',
    'require_durable_checkout_identity',
    'missing durable identity outcome',
  ],
  [
    'order.checkout_payment_identity_conflict',
    'checkout order identity conflicts with the payment settlement request',
    'validate_durable_checkout_identity',
    'durable identity conflict outcome',
  ],
  [
    'order.checkout_payment_state_conflict',
    'checkout order lifecycle does not allow payment settlement',
    'validate_payment_settlement_lifecycle',
    'local lifecycle conflict outcome',
  ],
  [
    'order.checkout_payment_reference_conflict',
    'checkout order is settled by another payment identity',
    'validate_settled_payment_identity',
    'settled payment identity outcome',
  ],
]) {
  requireText(mapper, `"${code}"`, `${label} code`);
  requireText(mapper, `"${message}"`, `${label} message`);
  requireText(mapper, `"${localOperation}"`, `${label} local operation`);
  requireText(settlement, `"${code}"`, `${label} preserved owner code`);
  requireText(settlement, `"${message}"`, `${label} preserved public message`);
}

for (const [value, label] of [
  ['match (error.code.as_str(), error.message.as_str()) {', 'exact code-and-message classification'],
  ['_ => return error,', 'non-local error passthrough'],
  ['let integrity_failure = matches!(', 'integrity severity classification'],
  ['"require_durable_checkout_identity"', 'missing identity error severity'],
  ['"validate_durable_checkout_identity"', 'identity conflict error severity'],
  ['"validate_settled_payment_identity"', 'payment identity error severity'],
  ['tracing::error!(', 'integrity error event'],
  ['tracing::warn!(', 'ordinary rejection warning event'],
  ['error = ?error', 'mapped error evidence'],
  ['owner = ORDER_PAYMENT_SETTLEMENT_OWNER', 'truthful owner diagnostic'],
  ['operation = SETTLE_PAYMENT_OPERATION', 'exact public operation diagnostic'],
  ['local_operation,', 'exact local operation diagnostic'],
  ['correlation_id = %context.correlation_id', 'correlation context'],
  ['tenant_id = %context.tenant_id', 'tenant context'],
  ['actor = ?context.actor', 'actor context'],
  ['channel = ?context.channel', 'channel context'],
  ['locale = %context.locale', 'locale context'],
  ['causation_id = ?context.causation_id', 'causation context'],
  ['traceparent = ?context.traceparent', 'trace context'],
  ['idempotency_key = ?context.idempotency_key', 'idempotency context'],
  ['deadline_ms = ?context.deadline_ms', 'deadline context'],
  ['internal_code = %error.code', 'stable code diagnostic'],
  ['internal_message = %error.message', 'stable message diagnostic'],
  ['error_kind = ?error.kind', 'typed error kind diagnostic'],
  ['retryable = error.retryable', 'retryability diagnostic'],
  ['boundary = ORDER_PAYMENT_SETTLEMENT_BOUNDARY', 'exact boundary diagnostic'],
  ['error\n}', 'same mapped error returned'],
]) requireText(mapper, value, label);

for (const [value, label] of [
  ['"order.checkout_payment_state_conflict"', 'service transition code remains present'],
  ['"order lifecycle conflicts with payment settlement"', 'service transition public message remains distinct'],
  ['OrderError::InvalidTransition { from, to }', 'service transition cause mapping remains intact'],
  ['order_error_to_port_error(&context, "mark_checkout_order_paid", error)', 'mark-paid service mapper remains intact'],
]) requireText(settlement, value, label);

requireText(
  ownerMapper,
  '"order lifecycle conflicts with payment settlement"',
  'service transition envelope remains outside local mapper classification',
);
forbidText(
  mapper,
  '"order lifecycle conflicts with payment settlement"',
  'service transition envelope must not be classified as a local lifecycle outcome',
);

for (const value of [
  'PortError::validation(',
  'PortError::conflict(',
  'PortError::new(',
  'PortError::unavailable(',
  'PortError::invariant_violation(',
]) forbidText(mapper, value, 'mapper must not construct a replacement public envelope');

for (const [pattern, expected, label] of [
  [/map_checkout_order_payment_settlement_local_port_error\(/g, 2, 'local mapper definition/use count'],
  [/"validate_request"/g, 1, 'request operation count'],
  [/"require_durable_checkout_identity"/g, 2, 'missing identity operation classification/severity count'],
  [/"validate_durable_checkout_identity"/g, 2, 'identity conflict operation classification/severity count'],
  [/"validate_payment_settlement_lifecycle"/g, 1, 'lifecycle operation count'],
  [/"validate_settled_payment_identity"/g, 2, 'payment identity operation classification/severity count'],
]) {
  const count = wrapper.match(pattern)?.length ?? 0;
  if (count !== expected) failures.push(`${label}: expected ${expected}, found ${count}`);
}

if (failures.length > 0) {
  console.error('Order payment settlement local context verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Order payment settlement local request, durable identity, lifecycle, and payment identity outcomes retain full delegated context and unchanged PortError envelopes',
);

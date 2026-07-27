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
const lib = read('crates/rustok-order/src/lib.rs');
const settlement = read('crates/rustok-order/src/checkout_payment_settlement.rs');
const compensation = read('crates/rustok-order/src/checkout_compensation.rs');
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

const compensationImpl = between(
  wrapper,
  'impl CheckoutOrderCompensationPort for InProcessCheckoutOrderCompensationPort {',
  'pub fn in_process_checkout_order_compensation_port(',
  'compensation wrapper implementation',
);
const settlementImpl = between(
  wrapper,
  'impl CheckoutOrderPaymentSettlementPort for InProcessCheckoutOrderPaymentSettlementPort {',
  'pub fn in_process_checkout_order_payment_settlement_port(',
  'settlement wrapper implementation',
);
const admission = between(
  wrapper,
  'fn require_order_checkout_write_admission(',
  'fn parse_order_tenant_id(',
  'shared order checkout admission helpers',
);
const tenant = between(
  wrapper,
  'fn parse_order_tenant_id(',
  'fn parse_order_actor_id(',
  'shared tenant validation',
);
const actor = between(
  wrapper,
  'fn parse_order_actor_id(',
  'fn require_order_checkout_causation(',
  'shared actor validation',
);
const causation = between(
  wrapper,
  'fn require_order_checkout_causation(',
  'fn log_order_checkout_context_rejection(',
  'shared causation validation',
);
const contextLog = wrapper.slice(wrapper.indexOf('fn log_order_checkout_context_rejection('));

for (const [value, label] of [
  ['mod checkout_compensation;', 'private compensation implementation module'],
  ['mod checkout_payment_settlement;', 'private settlement implementation module'],
  ['pub mod checkout_owner_context;', 'public context wrapper module'],
  ['CheckoutOrderCompensationPort, CheckoutOrderCompensationRequest,', 'compensation contract re-export'],
  ['CheckoutOrderCompensationSnapshot,', 'compensation snapshot re-export'],
  ['CheckoutOrderPaymentSettlementPort, SettleCheckoutOrderPaymentRequest,', 'settlement contract re-export'],
  ['InProcessCheckoutOrderCompensationPort, InProcessCheckoutOrderPaymentSettlementPort,', 'wrapper struct re-export'],
  ['in_process_checkout_order_compensation_port,', 'compensation factory re-export'],
  ['in_process_checkout_order_payment_settlement_port,', 'settlement factory re-export'],
]) requireText(lib, value, label);

for (const value of [
  'pub mod checkout_compensation;',
  'pub mod checkout_payment_settlement;',
  'pub use checkout_compensation::*;',
  'pub use checkout_payment_settlement::*;',
]) forbidText(lib, value, 'public context-bypass path');

for (const [value, label] of [
  ['const ORDER_COMPENSATION_OWNER: &str = "rustok_order.checkout_compensation";', 'compensation owner'],
  ['const ORDER_COMPENSATION_BOUNDARY: &str = "checkout_order_compensation_port";', 'compensation boundary'],
  ['const COMPENSATE_OPERATION: &str = "compensate_checkout_order";', 'compensation operation'],
  ['const ORDER_PAYMENT_SETTLEMENT_OWNER: &str = "rustok_order.checkout_payment_settlement";', 'settlement owner'],
  ['const ORDER_PAYMENT_SETTLEMENT_BOUNDARY: &str = "checkout_order_payment_settlement_port";', 'settlement boundary'],
  ['const SETTLE_PAYMENT_OPERATION: &str = "settle_checkout_payment";', 'settlement operation'],
  ['pub struct InProcessCheckoutOrderCompensationPort {', 'compensation wrapper struct'],
  ['pub struct InProcessCheckoutOrderPaymentSettlementPort {', 'settlement wrapper struct'],
  ['inner: Arc<dyn CheckoutOrderCompensationPort>', 'compensation inner port'],
  ['inner: Arc<dyn CheckoutOrderPaymentSettlementPort>', 'settlement inner port'],
  ['pub fn with_identity_port(', 'identity-port constructor parity'],
]) requireText(wrapper, value, label);

for (const [block, values, label] of [
  [
    compensationImpl,
    [
      'require_order_checkout_write_admission(',
      'ORDER_COMPENSATION_OWNER,',
      'ORDER_COMPENSATION_BOUNDARY,',
      'parse_order_tenant_id(',
      'parse_order_actor_id(',
      'require_order_checkout_causation(',
      '"order.checkout_compensation_causation_invalid",',
      'request.checkout_operation_id,',
      'self.inner.compensate_checkout_order(context, request).await',
    ],
    'compensation wrapper routing',
  ],
  [
    settlementImpl,
    [
      'require_order_checkout_write_admission(',
      'ORDER_PAYMENT_SETTLEMENT_OWNER,',
      'ORDER_PAYMENT_SETTLEMENT_BOUNDARY,',
      'parse_order_tenant_id(',
      'parse_order_actor_id(',
      'require_order_checkout_causation(',
      '"order.checkout_payment_causation_invalid",',
      'request.checkout_operation_id,',
      'self.inner.settle_checkout_payment(context, request).await',
    ],
    'settlement wrapper routing',
  ],
]) {
  for (const value of values) requireText(block, value, label);
  const admissionIndex = block.indexOf('require_order_checkout_write_admission(');
  const tenantIndex = block.indexOf('parse_order_tenant_id(');
  const actorIndex = block.indexOf('parse_order_actor_id(');
  const causationIndex = block.indexOf('require_order_checkout_causation(');
  const delegationIndex = block.indexOf('self.inner.');
  if (!(admissionIndex < tenantIndex && tenantIndex < actorIndex && actorIndex < causationIndex && causationIndex < delegationIndex)) {
    failures.push(`${label}: expected admission -> tenant -> actor -> causation -> delegation ordering`);
  }
}

for (const [value, label] of [
  ['context.require_policy(PortCallPolicy::write()).map_err(|error| {', 'write-policy interception'],
  ['context.require_write_semantics().map_err(|error| {', 'write-semantics interception'],
  ['"policy",', 'policy phase'],
  ['"write_semantics",', 'write-semantics phase'],
  ['PortErrorKind::Unavailable | PortErrorKind::Timeout | PortErrorKind::InvariantViolation', 'technical severity classification'],
  ['tracing::error!(', 'technical admission event'],
  ['tracing::warn!(', 'ordinary admission event'],
  ['error = ?error', 'original admission error evidence'],
  ['admission_phase,', 'admission phase evidence'],
  ['internal_code = %error.code', 'admission stable code'],
  ['internal_message = %error.message', 'admission stable message'],
  ['error_kind = ?error.kind', 'admission typed kind'],
  ['retryable = error.retryable', 'admission retryability'],
  ['error\n    })', 'same admission error return'],
]) requireText(admission, value, label);

for (const [block, phase, code, label] of [
  [tenant, 'tenant_id', 'order.tenant_id_invalid', 'tenant validation'],
  [actor, 'actor_id', 'order.actor_id_invalid', 'actor validation'],
  [causation, 'causation_id', 'checkout operation context is invalid', 'causation validation'],
]) {
  requireText(block, `"${phase}"`, `${label} phase`);
  requireText(block, code, `${label} stable envelope`);
  requireText(block, 'let error = PortError::validation(', `${label} stable error construction`);
  if (label !== 'causation validation') requireText(block, 'Some(&cause)', `${label} parse cause retention`);
}

for (const [value, label] of [
  ['parse_cause = ?parse_cause', 'parse-cause evidence'],
  ['error = ?error', 'mapped validation error'],
  ['owner,', 'truthful owner'],
  ['operation,', 'exact operation'],
  ['validation_phase,', 'validation phase'],
  ['correlation_id = %context.correlation_id', 'correlation context'],
  ['tenant_id = %context.tenant_id', 'tenant context'],
  ['actor = ?context.actor', 'actor context'],
  ['channel = ?context.channel', 'channel context'],
  ['locale = %context.locale', 'locale context'],
  ['causation_id = ?context.causation_id', 'causation context'],
  ['traceparent = ?context.traceparent', 'trace context'],
  ['idempotency_key = ?context.idempotency_key', 'idempotency context'],
  ['deadline_ms = ?context.deadline_ms', 'deadline context'],
  ['expected_checkout_operation_id = ?expected_checkout_operation_id', 'expected operation identity'],
  ['internal_code = %error.code', 'mapped code'],
  ['internal_message = %error.message', 'mapped message'],
  ['error_kind = ?error.kind', 'mapped kind'],
  ['retryable = error.retryable', 'mapped retryability'],
  ['boundary,', 'exact boundary'],
]) requireText(contextLog, value, label);

for (const [value, label] of [
  ['checkout_compensation::in_process_checkout_order_compensation_port(', 'legacy compensation delegation'],
  ['checkout_payment_settlement::in_process_checkout_order_payment_settlement_port(', 'legacy settlement delegation'],
  ['checkout_compensation::InProcessCheckoutOrderCompensationPort::with_identity_port(', 'compensation constructor parity'],
  ['checkout_payment_settlement::InProcessCheckoutOrderPaymentSettlementPort::with_identity_port(', 'settlement constructor parity'],
]) requireText(wrapper, value, label);

for (const [content, values, label] of [
  [
    settlement,
    [
      'fn validate_request(',
      'order_error_to_port_error(&context, "mark_checkout_order_paid", error)',
      '"order.checkout_payment_identity_missing"',
      '"order.checkout_payment_identity_conflict"',
      '"order.checkout_payment_state_conflict"',
      '"order.checkout_payment_reference_conflict"',
    ],
    'preserved settlement behavior',
  ],
  [
    compensation,
    [
      'fn validate_identity(',
      'fn manual_reconciliation(',
      '"read_checkout_order_for_compensation"',
      '"cancel_checkout_order"',
      '"order.checkout_compensation_identity_invalid"',
      '"order.checkout_compensation_identity_conflict"',
      '"order.checkout_compensation_manual_reconciliation"',
    ],
    'preserved compensation behavior',
  ],
]) for (const value of values) requireText(content, value, label);

for (const [pattern, expected, label] of [
  [/impl CheckoutOrderCompensationPort for InProcessCheckoutOrderCompensationPort/g, 1, 'compensation wrapper impl count'],
  [/impl CheckoutOrderPaymentSettlementPort for InProcessCheckoutOrderPaymentSettlementPort/g, 1, 'settlement wrapper impl count'],
  [/require_order_checkout_write_admission\(/g, 3, 'shared admission definition/use count'],
  [/parse_order_tenant_id\(/g, 3, 'tenant validation definition/use count'],
  [/parse_order_actor_id\(/g, 3, 'actor validation definition/use count'],
  [/require_order_checkout_causation\(/g, 3, 'causation validation definition/use count'],
]) {
  const count = wrapper.match(pattern)?.length ?? 0;
  if (count !== expected) failures.push(`${label}: expected ${expected}, found ${count}`);
}

if (failures.length > 0) {
  console.error('Order checkout owner context verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Public order checkout factories retain full admission and delegated context before preserving the existing settlement and compensation implementations',
);

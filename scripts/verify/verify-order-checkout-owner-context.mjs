#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const read = (relativePath) => readFileSync(new URL(relativePath, root), 'utf8');

const shared = read('crates/rustok-order/src/checkout_owner_context.rs');
const compensationLocal = read('crates/rustok-order/src/checkout_compensation_local_context.rs');
const settlement = read('crates/rustok-order/src/checkout_payment_settlement.rs');
const compensation = read('crates/rustok-order/src/checkout_compensation.rs');
const lib = read('crates/rustok-order/src/lib.rs');
const doc = read('crates/rustok-order/docs/checkout-owner-context.md');
const evidence = JSON.parse(
  read('crates/rustok-order/contracts/evidence/checkout-compensation-diagnostic-safety-source.json'),
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

for (const marker of [
  'mod checkout_compensation;',
  'mod checkout_payment_settlement;',
  'mod checkout_compensation_local_context;',
  '#[path = "checkout_owner_context.rs"]',
  'mod checkout_owner_context_impl;',
  'pub mod checkout_owner_context {',
  'pub use crate::checkout_compensation_local_context::{',
  'pub use crate::checkout_owner_context_impl::{',
  'pub use checkout_compensation_local_context::{',
  'pub use checkout_owner_context_impl::{',
]) requireText(lib, marker, 'public checkout facade');

const compensationImpl = between(
  shared,
  'impl CheckoutOrderCompensationPort for InProcessCheckoutOrderCompensationPort {',
  'pub fn in_process_checkout_order_compensation_port(',
  'compensation shared wrapper',
);
const settlementImpl = between(
  shared,
  'impl CheckoutOrderPaymentSettlementPort for InProcessCheckoutOrderPaymentSettlementPort {',
  'pub fn in_process_checkout_order_payment_settlement_port(',
  'settlement shared wrapper',
);
for (const [block, delegation, label] of [
  [compensationImpl, 'self.inner.compensate_checkout_order(context, request).await', 'compensation'],
  [settlementImpl, 'self.inner.settle_checkout_payment(context, request).await', 'settlement'],
]) {
  const order = [
    block.indexOf('require_order_checkout_write_admission('),
    block.indexOf('parse_order_tenant_id('),
    block.indexOf('parse_order_actor_id('),
    block.indexOf('require_order_checkout_causation('),
    block.indexOf(delegation),
  ];
  if (!order.every((value, index) => value >= 0 && (index === 0 || order[index - 1] < value))) {
    failures.push(`${label} must preserve admission -> tenant -> actor -> causation -> delegation order`);
  }
}

for (const marker of [
  'struct OrderCheckoutContextFacts',
  'fn order_checkout_context_facts(',
  'tenant_id_length: context.tenant_id.chars().count()',
  'actor_kind',
  'actor_id_length: context.actor.id.chars().count()',
  'claim_count: context.claims.len()',
  'role_count: context.roles.len()',
  'channel_present: context.channel.is_some()',
  'channel_length: context.channel.as_ref().map(|value| value.chars().count())',
  'locale_length: context.locale.chars().count()',
  'causation_id_present: context.causation_id.is_some()',
  'traceparent_present: context.traceparent.is_some()',
  'idempotency_key_present: context.idempotency_key.is_some()',
  'deadline_ms: context.deadline_ms',
]) requireText(shared, marker, 'safe shared context shape');

for (const marker of [
  'context.require_policy(PortCallPolicy::write())',
  'context.require_write_semantics()',
  'log_order_checkout_admission_rejection(',
  'PortErrorKind::Unavailable | PortErrorKind::Timeout | PortErrorKind::InvariantViolation',
  'tracing::error!(',
  'tracing::warn!(',
  'internal_code = %error.code',
  'internal_message = %error.message',
  'error_kind = ?error.kind',
  'retryable = error.retryable',
]) requireText(shared, marker, 'preserved admission mapping');

for (const marker of [
  'parse_cause = ?evidence.parse_cause',
  'expected_checkout_operation_id_present',
  'expected_checkout_operation_id_non_nil',
  'causation_matches = false',
  'let error = PortError::validation(',
  'return Err(error);',
]) requireText(shared, marker, 'safe context rejection');

for (const [code, operation] of [
  ['order.checkout_payment_request_invalid', 'validate_request'],
  ['order.checkout_payment_identity_missing', 'require_durable_checkout_identity'],
  ['order.checkout_payment_identity_conflict', 'validate_durable_checkout_identity'],
  ['order.checkout_payment_state_conflict', 'validate_payment_settlement_lifecycle'],
  ['order.checkout_payment_reference_conflict', 'validate_settled_payment_identity'],
]) {
  requireText(shared, `"${code}"`, `settlement wrapper code ${code}`);
  requireText(shared, `"${operation}"`, `settlement wrapper operation ${operation}`);
}
for (const marker of [
  'fn checkout_order_payment_settlement_local_operation(code: &str)',
  'checkout_order_payment_settlement_local_operation(error.code.as_str())',
  '_ => None,',
  '\n    error\n}',
]) requireText(shared, marker, 'stable-code settlement local mapper');
forbidText(shared, 'match (error.code.as_str(), error.message.as_str())', 'message-pair settlement classifier');

for (const [content, label] of [
  [shared, 'shared checkout context'],
  [compensationLocal, 'compensation local wrapper'],
]) {
  for (const forbidden of [
    'tenant_id = %context.tenant_id',
    'actor = ?context.actor',
    'channel = ?context.channel',
    'locale = %context.locale',
    'causation_id = ?context.causation_id',
    'traceparent = ?context.traceparent',
    'idempotency_key = ?context.idempotency_key',
    'expected_checkout_operation_id = ?evidence.expected_checkout_operation_id',
  ]) forbidText(content, forbidden, `${label} raw field`);
}

for (const marker of [
  'checkout_compensation::in_process_checkout_order_compensation_port(',
  'checkout_payment_settlement::in_process_checkout_order_payment_settlement_port(',
  'checkout_compensation::InProcessCheckoutOrderCompensationPort::with_identity_port(',
  'checkout_payment_settlement::InProcessCheckoutOrderPaymentSettlementPort::with_identity_port(',
]) requireText(shared, marker, 'preserved inner construction');
for (const marker of [
  'fn validate_request(',
  'order_error_to_port_error(&context, "mark_checkout_order_paid", error)',
  '"order.checkout_payment_identity_missing"',
  '"order.checkout_payment_identity_conflict"',
  '"order.checkout_payment_state_conflict"',
  '"order.checkout_payment_reference_conflict"',
]) requireText(settlement, marker, 'settlement business owner unchanged');
for (const marker of [
  'fn validate_identity(',
  'fn manual_reconciliation(',
  '"read_checkout_order_for_compensation"',
  '"cancel_checkout_order"',
]) requireText(compensation, marker, 'compensation owner preserved');

if (evidence.source_contract?.shared_admission_safe_context_shape !== true) {
  failures.push('evidence shared_admission_safe_context_shape must be true');
}
if (evidence.source_contract?.write_admission_order_changed !== false) {
  failures.push('evidence write_admission_order_changed must be false');
}
if (evidence.source_contract?.payment_settlement_business_source_changed !== false) {
  failures.push('evidence payment_settlement_business_source_changed must be false');
}

for (const marker of [
  'Status: **source-ready / unvalidated**',
  'Both public wrappers preserve:',
  'Raw tenant, actor, channel, locale, causation, traceparent, and idempotency values are',
  '`checkout_payment_settlement.rs` is not modified by this slice.',
]) requireText(doc, marker, 'owner context documentation');

if (failures.length > 0) {
  console.error('Order checkout owner-context diagnostic-safety verification failed:');
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  'Order checkout wrappers preserve admission/context ordering and use safe shape plus stable-code local attribution while returning the same PortError',
);

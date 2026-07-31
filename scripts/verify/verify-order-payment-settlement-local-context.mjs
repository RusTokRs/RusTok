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
const doc = read('crates/rustok-order/docs/checkout-payment-settlement-local-context.md');
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

const wrapperImpl = between(
  wrapper,
  'impl CheckoutOrderPaymentSettlementPort for InProcessCheckoutOrderPaymentSettlementPort {',
  'pub fn in_process_checkout_order_payment_settlement_port(',
  'settlement wrapper',
);
const mapper = between(
  wrapper,
  'fn checkout_order_payment_settlement_local_operation(',
  'fn require_order_checkout_write_admission(',
  'settlement local mapper',
);

for (const marker of [
  'let diagnostic_context = context.clone();',
  'let result = self.inner.settle_checkout_payment(context, request).await;',
  'map_checkout_order_payment_settlement_local_port_error(&diagnostic_context, error)',
]) requireText(wrapperImpl, marker, 'preserved settlement delegation');

for (const [code, operation] of [
  ['order.checkout_payment_request_invalid', 'validate_request'],
  ['order.checkout_payment_identity_missing', 'require_durable_checkout_identity'],
  ['order.checkout_payment_identity_conflict', 'validate_durable_checkout_identity'],
  ['order.checkout_payment_state_conflict', 'validate_payment_settlement_lifecycle'],
  ['order.checkout_payment_reference_conflict', 'validate_settled_payment_identity'],
]) {
  requireText(mapper, `"${code}"`, `settlement code ${code}`);
  requireText(mapper, `"${operation}"`, `settlement operation ${operation}`);
  requireText(settlement, `"${code}"`, `owner code ${code}`);
}
for (const marker of [
  'fn checkout_order_payment_settlement_local_operation(code: &str)',
  'match code {',
  'checkout_order_payment_settlement_local_operation(error.code.as_str())',
  '_ => None,',
  'let integrity_failure = matches!(',
  'tracing::error!(',
  'tracing::warn!(',
  'error = ?error',
  'tenant_id_length = context_facts.tenant_id_length',
  'actor_kind = context_facts.actor_kind',
  'channel_present = context_facts.channel_present',
  'locale_length = context_facts.locale_length',
  'causation_id_present = context_facts.causation_id_present',
  'idempotency_key_present = context_facts.idempotency_key_present',
  'boundary = ORDER_PAYMENT_SETTLEMENT_BOUNDARY',
  '\n    error\n}',
]) requireText(mapper, marker, 'safe stable-code settlement mapper');

for (const forbidden of [
  'error.message.as_str()',
  'match (error.code.as_str(), error.message.as_str())',
  'tenant_id = %context.tenant_id',
  'actor = ?context.actor',
  'channel = ?context.channel',
  'locale = %context.locale',
  'causation_id = ?context.causation_id',
  'traceparent = ?context.traceparent',
  'idempotency_key = ?context.idempotency_key',
]) forbidText(mapper, forbidden, 'settlement local unsafe diagnostic');

for (const value of [
  'PortError::validation(',
  'PortError::conflict(',
  'PortError::new(',
  'PortError::unavailable(',
  'PortError::invariant_violation(',
]) forbidText(mapper, value, 'mapper replacement envelope');

for (const marker of [
  'fn validate_request(',
  'order_error_to_port_error(&context, "mark_checkout_order_paid", error)',
  '"checkout payment settlement request is invalid"',
  '"checkout requires manual reconciliation"',
  '"checkout order identity conflicts with the payment settlement request"',
  '"checkout order lifecycle does not allow payment settlement"',
  '"checkout order is settled by another payment identity"',
  '"order lifecycle conflicts with payment settlement"',
]) requireText(settlement, marker, 'settlement business/public contract preserved');

if (evidence.source_contract?.payment_settlement_business_source_changed !== false) {
  failures.push('evidence payment_settlement_business_source_changed must be false');
}
if (evidence.validation?.tests_run !== false || evidence.validation?.compile_proven !== false) {
  failures.push('settlement collateral evidence must remain unvalidated');
}

for (const marker of [
  'Status: **source-ready / unvalidated**',
  'Human-readable `PortError.message` is not used as control flow.',
  '`checkout_payment_settlement.rs` is not modified by this slice.',
  'Owner-local settlement diagnostics remain a separate cleanup.',
]) requireText(doc, marker, 'settlement local documentation');

if (failures.length > 0) {
  console.error('Order payment settlement local diagnostic-safety verification failed:');
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  'Order payment settlement local mapper uses stable-code attribution and safe context while preserving owner business behavior and the same PortError',
);

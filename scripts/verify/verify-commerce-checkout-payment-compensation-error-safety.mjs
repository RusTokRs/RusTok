#!/usr/bin/env node

import { readFileSync } from 'node:fs';

const root = new URL('../../', import.meta.url);
const read = (path) => readFileSync(new URL(path, root), 'utf8');
const failures = [];
const requireText = (source, value, label) => {
  if (!source.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (source, value, label) => {
  if (source.includes(value)) failures.push(`${label}: forbidden ${value}`);
};

const services = read('crates/rustok-commerce/src/services/mod.rs');
const facade = read('crates/rustok-commerce/src/services/checkout_compensation_error_safe.rs');
const retained = read('crates/rustok-commerce/src/services/checkout_compensation_owner_ports.rs');
const owner = read('crates/rustok-payment/src/checkout_compensation_context.rs');
const evidence = JSON.parse(read(
  'crates/rustok-commerce/contracts/evidence/checkout-payment-compensation-error-safety-source-review.json',
));
const doc = read('crates/rustok-commerce/docs/checkout-payment-compensation-error-safety.md');

requireText(services, '#[path = "checkout_compensation_error_safe.rs"]\nmod checkout_compensation;', 'mounted facade');
for (const marker of [
  'use super::rustok_payment_shim as rustok_payment;',
  'CheckoutPaymentCompensationPort as CanonicalCheckoutPaymentCompensationPort',
  'BoundaryFacts::payment(&request)',
  'in_process_checkout_payment_compensation_port(',
  'InProcessCheckoutPaymentCompensationPort::with_provider_registry(',
  'wrap_checkout_payment_compensation_port(',
  'with_payment_compensation_port(',
  '"Checkout payment compensation requires manual reconciliation"',
  '"Checkout payment compensation request is invalid"',
  '"Checkout payment compensation resource was not found"',
  '"Checkout payment compensation conflicts with the current payment state"',
  '"Checkout payment compensation is not permitted"',
  '"Checkout payment compensation service is temporarily unavailable"',
  '"Checkout payment compensation could not be completed safely"',
  'kind: error.kind',
  'code: error.code',
  'retryable: error.retryable',
]) requireText(facade, marker, 'payment adapter');

for (const marker of [
  'error = ?error',
  'internal_message = %error.message',
  'correlation_id = %context.correlation_id',
]) forbidText(facade, marker, 'raw mounted payment diagnostics');

for (const marker of [
  'compensate_checkout_payment(',
  'PAYMENT_MANUAL_RECONCILIATION_CODE',
  'let message = compensation.to_string();',
  'mark_compensation_retryable(',
]) requireText(retained, marker, 'retained payment flow');
for (const marker of [
  'payment.checkout_compensation_manual_reconciliation',
  'compensate_checkout_payment(',
]) requireText(owner, marker, 'payment owner contract');

for (const [key, expected] of Object.entries({
  combined_payment_order_inventory_cart_facade_active: true,
  payment_owner_message_public: false,
  payment_owner_message_persisted: false,
  legacy_payment_diagnostic_suppressed: true,
  legacy_cart_diagnostic_suppressed: true,
  cart_compensation_mapper_changed: true,
  broad_ecommerce_cleanup_closed: false,
  runtime_evidence_claimed: false,
})) {
  if (evidence.source_contract?.[key] !== expected) {
    failures.push(`evidence source_contract.${key} must be ${expected}`);
  }
}
for (const [key, value] of Object.entries(evidence.validation ?? {})) {
  if (value !== false) failures.push(`evidence validation.${key} must remain false`);
}
if (!Array.isArray(evidence.execution) || evidence.execution.length !== 0) {
  failures.push('evidence execution must remain empty');
}
requireText(doc, 'Status: **source-reviewed / unvalidated**', 'truthful payment documentation');

if (failures.length > 0) {
  console.error('Checkout payment compensation error-safety verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log('✔ Checkout payment compensation remains guarded in the four-owner facade');

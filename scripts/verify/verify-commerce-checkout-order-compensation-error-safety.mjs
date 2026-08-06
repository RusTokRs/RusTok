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
const owner = read('crates/rustok-order/src/checkout_compensation.rs');
const evidence = JSON.parse(read(
  'crates/rustok-commerce/contracts/evidence/checkout-order-compensation-error-safety-source-review.json',
));
const doc = read('crates/rustok-commerce/docs/checkout-order-compensation-error-safety.md');

requireText(services, '#[path = "checkout_compensation_error_safe.rs"]\nmod checkout_compensation;', 'mounted facade');
for (const marker of [
  'use super::rustok_order_shim as rustok_order;',
  'CheckoutOrderCompensationPort as CanonicalCheckoutOrderCompensationPort',
  'BoundaryFacts::order(&request)',
  'in_process_checkout_order_compensation_port(',
  'InProcessCheckoutOrderCompensationPort::with_identity_port(',
  'wrap_checkout_order_compensation_port(',
  'with_order_compensation_port(',
  '"Checkout order compensation requires manual reconciliation"',
  '"Checkout order compensation request is invalid"',
  '"Checkout order compensation resource was not found"',
  '"Checkout order compensation conflicts with the current order state"',
  '"Checkout order compensation is not permitted"',
  '"Checkout order compensation service is temporarily unavailable"',
  '"Checkout order compensation could not be completed safely"',
  'kind: error.kind',
  'code: error.code',
  'retryable: error.retryable',
]) requireText(facade, marker, 'order adapter');

for (const marker of [
  'error = ?error',
  'internal_message = %error.message',
  'correlation_id = %context.correlation_id',
]) forbidText(facade, marker, 'raw mounted order diagnostics');

for (const marker of [
  'compensate_checkout_order(',
  'ORDER_MANUAL_RECONCILIATION_CODE',
  'let message = compensation.to_string();',
  'mark_compensation_retryable(',
]) requireText(retained, marker, 'retained order flow');
for (const marker of [
  'trait CheckoutOrderCompensationPort',
  'compensate_checkout_order(',
  'order.checkout_compensation_manual_reconciliation',
]) requireText(owner, marker, 'order owner contract');

for (const [key, expected] of Object.entries({
  mounted_combined_payment_order_inventory_cart_facade_active: true,
  order_owner_message_public: false,
  order_owner_message_persisted: false,
  legacy_order_diagnostic_suppressed: true,
  legacy_cart_diagnostic_suppressed: true,
  cart_compensation_mapper_changed: true,
  remaining_compensation_cleanup_closed: true,
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
requireText(doc, 'Status: **source-reviewed / unvalidated**', 'truthful order documentation');

if (failures.length > 0) {
  console.error('Checkout order compensation error-safety verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log('✔ Checkout order compensation remains guarded in the four-owner facade');

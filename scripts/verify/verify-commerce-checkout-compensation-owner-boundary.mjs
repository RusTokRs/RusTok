#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

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
const compensationFacade = read(
  'crates/rustok-commerce/src/services/checkout_compensation_payment_safe.rs',
);
const compensationLegacy = read(
  'crates/rustok-commerce/src/services/checkout_compensation_owner_ports.rs',
);
const compensation = `${compensationFacade}\n${compensationLegacy}`;
const order = read('crates/rustok-order/src/checkout_compensation.rs');
const payment = read('crates/rustok-payment/src/checkout_compensation.rs');

requireText(
  services,
  '#[path = "checkout_compensation_payment_safe.rs"]',
  'commerce services mount',
);
forbidText(
  services.replace(
    '#[path = "checkout_compensation_payment_safe.rs"]\nmod checkout_compensation;',
    '',
  ),
  'mod checkout_compensation;',
  'commerce services mount',
);
requireText(
  compensationFacade,
  'include!("checkout_compensation_owner_ports.rs");',
  'private retained compensation source',
);
requireText(
  compensationFacade,
  'wrap_checkout_payment_compensation_port(',
  'mounted payment compensation adapter',
);

for (const value of [
  'CheckoutOrderCompensationPort',
  'CheckoutPaymentCompensationPort',
  'compensate_checkout_order(',
  'compensate_checkout_payment(',
  'with_causation_id(',
  'with_idempotency_key(',
  'with_deadline(',
]) {
  requireText(compensation, value, 'mounted commerce compensation');
}
for (const value of [
  'OrderService',
  'PaymentService',
  'PaymentProviderOperationJournal',
  'PaymentOrchestrationService',
  'CancelPaymentInput',
  '.cancel_order(',
  '.cancel_collection(',
]) {
  forbidText(compensation, value, 'mounted commerce compensation');
}

for (const [source, label, traitName, operation] of [
  [
    order,
    'order compensation owner',
    'CheckoutOrderCompensationPort',
    'compensate_checkout_order',
  ],
  [
    payment,
    'payment compensation owner',
    'CheckoutPaymentCompensationPort',
    'compensate_checkout_payment',
  ],
]) {
  requireText(source, `trait ${traitName}`, label);
  requireText(source, `async fn ${operation}(`, label);
  requireText(source, 'require_policy(PortCallPolicy::write())?', label);
  requireText(source, 'require_write_semantics()?', label);
  requireText(source, 'checkout_operation_id', label);
}

requireText(order, 'OrderService::new(', 'order compensation owner');
requireText(payment, 'PaymentService::new(', 'payment compensation owner');
requireText(
  payment,
  'PaymentProviderOperationJournal::new(',
  'payment compensation owner',
);
requireText(payment, 'execute_cancel(', 'payment compensation owner');
requireText(
  payment,
  'PROVIDER_OPERATION_RECONCILIATION_REQUIRED',
  'payment compensation owner',
);

if (failures.length > 0) {
  console.error('Checkout compensation owner-boundary verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Checkout compensation is mounted through typed order/payment owner ports and the payment-safe consumer adapter',
);

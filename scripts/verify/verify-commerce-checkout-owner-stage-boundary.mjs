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
const paymentStage = read(
  'crates/rustok-commerce/src/services/checkout_payment_stages.rs',
);
const fulfillmentStage = read(
  'crates/rustok-commerce/src/services/checkout_fulfillment_stages.rs',
);
const pipeline = read(
  'crates/rustok-commerce/src/services/checkout_stage_pipeline_owner_ports.rs',
);
const paymentOwner = read('crates/rustok-payment/src/checkout_execution.rs');
const fulfillmentOwner = read('crates/rustok-fulfillment/src/checkout_execution.rs');
const orderOwner = read(
  'crates/rustok-order/src/checkout_payment_settlement.rs',
);

requireText(
  services,
  '#[path = "checkout_stage_pipeline_owner_ports.rs"]',
  'commerce services mount',
);
for (const [source, label, required] of [
  [
    paymentStage,
    'payment stage',
    [
      'CheckoutPaymentExecutionPort',
      'prepare_checkout_collection(',
      'authorize_checkout_collection(',
      'capture_checkout_collection(',
      'read_checkout_collection(',
    ],
  ],
  [
    fulfillmentStage,
    'fulfillment stage',
    [
      'CheckoutFulfillmentExecutionPort',
      'CheckoutOrderPaymentSettlementPort',
      'ensure_checkout_fulfillments(',
      'read_checkout_fulfillments(',
      'settle_checkout_payment(',
    ],
  ],
  [
    pipeline,
    'mounted pipeline',
    [
      'payment_stage.load_payment_captured_state',
      'fulfillment_stage.load_fulfillment_created_state',
    ],
  ],
]) {
  for (const value of required) requireText(source, value, label);
  requireText(source, 'with_causation_id(', label);
  requireText(source, 'with_deadline(', label);
}

for (const [source, label] of [
  [paymentStage, 'payment stage'],
  [fulfillmentStage, 'fulfillment stage'],
  [pipeline, 'mounted pipeline'],
]) {
  for (const value of [
    'PaymentService',
    'FulfillmentService',
    'OrderService',
    'PaymentProviderOperationJournal',
    'PaymentOrchestrationService',
    'FROM fulfillments',
    'SELECT id FROM fulfillments',
  ]) {
    forbidText(source, value, label);
  }
}

for (const [source, label, port, operation] of [
  [
    paymentOwner,
    'payment owner',
    'CheckoutPaymentExecutionPort',
    'execute_journaled_provider_operation',
  ],
  [
    fulfillmentOwner,
    'fulfillment owner',
    'CheckoutFulfillmentExecutionPort',
    'create_fulfillment(',
  ],
  [
    orderOwner,
    'order owner',
    'CheckoutOrderPaymentSettlementPort',
    'mark_paid(',
  ],
]) {
  requireText(source, `trait ${port}`, label);
  requireText(source, operation, label);
  requireText(source, 'require_policy(PortCallPolicy::', label);
}

for (const key of [
  'payment_collection:{}:authorize',
  'payment_collection:{}:capture',
]) {
  requireText(paymentOwner, key, 'payment owner canonical provider identity');
}
requireText(
  paymentOwner,
  'authorize_payment_collection',
  'payment owner legacy provider payload',
);
requireText(
  paymentOwner,
  'capture_payment_collection',
  'payment owner legacy provider payload',
);

if (failures.length > 0) {
  console.error('Checkout owner-stage boundary verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Checkout payment, fulfillment, order settlement, and pipeline recovery use owner boundaries',
);

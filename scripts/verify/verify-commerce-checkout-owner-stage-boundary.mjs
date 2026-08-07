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
const orderStageFacade = read(
  'crates/rustok-commerce/src/services/checkout_order_stages.rs',
);
const orderStageLegacy = read(
  'crates/rustok-commerce/src/services/checkout_order_stages_legacy.rs',
);
const orderStage = `${orderStageFacade}\n${orderStageLegacy}`;
const paymentStageFacade = read(
  'crates/rustok-commerce/src/services/checkout_payment_stages.rs',
);
const paymentStageLegacy = read(
  'crates/rustok-commerce/src/services/checkout_payment_stages_legacy.rs',
);
const paymentStage = `${paymentStageFacade}\n${paymentStageLegacy}`;
const fulfillmentStageFacade = read(
  'crates/rustok-commerce/src/services/checkout_fulfillment_stages.rs',
);
const fulfillmentStageLegacy = read(
  'crates/rustok-commerce/src/services/checkout_fulfillment_stages_legacy.rs',
);
const fulfillmentStage = `${fulfillmentStageFacade}\n${fulfillmentStageLegacy}`;
const pipeline = read(
  'crates/rustok-commerce/src/services/checkout_stage_pipeline_owner_ports.rs',
);
const orderCompletionOwner = read('crates/rustok-order/src/ports.rs');
const orderRecoveryOwner = read('crates/rustok-order/src/checkout_order_recovery.rs');
const paymentOwner = read('crates/rustok-payment/src/checkout_execution.rs');
const fulfillmentOwner = read('crates/rustok-fulfillment/src/checkout_execution.rs');
const orderSettlementOwner = read(
  'crates/rustok-order/src/checkout_payment_settlement.rs',
);

requireText(
  services,
  '#[path = "checkout_stage_pipeline_owner_ports.rs"]',
  'commerce services mount',
);
requireText(
  orderStageFacade,
  'include!("checkout_order_stages_legacy.rs");',
  'mounted order-stage facade',
);
requireText(
  orderStageFacade,
  'struct SanitizingCheckoutCompletionPort',
  'mounted order completion adapter',
);
requireText(
  orderStageFacade,
  'pub struct CheckoutOrderRecoveryAdapter',
  'mounted order recovery/read adapter',
);
requireText(
  paymentStageFacade,
  'include!("checkout_payment_stages_legacy.rs");',
  'mounted payment-stage facade',
);
requireText(
  paymentStageFacade,
  'struct SanitizingCheckoutPaymentExecutionPort',
  'mounted payment-stage owner adapter',
);
requireText(
  fulfillmentStageFacade,
  'include!("checkout_fulfillment_stages_legacy.rs");',
  'mounted fulfillment-stage facade',
);
requireText(
  fulfillmentStageFacade,
  'struct SanitizingCheckoutFulfillmentExecutionPort',
  'mounted fulfillment-stage owner adapter',
);
requireText(
  fulfillmentStageFacade,
  'struct SanitizingCheckoutOrderPaymentSettlementPort',
  'mounted order-settlement owner adapter',
);

for (const [source, label, required] of [
  [
    orderStage,
    'order stage',
    [
      'CheckoutCompletionPort',
      'CheckoutOrderRecoveryAdapter',
      'recover_existing_checkout(',
      'complete_checkout(',
      'read_checkout_order(',
    ],
  ],
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
      'self.order_stage\n            .load_payment_ready_state',
      'self.payment_stage\n            .load_payment_captured_state',
      'self.fulfillment_stage\n            .load_fulfillment_created_state',
    ],
  ],
]) {
  for (const value of required) requireText(source, value, label);
  requireText(source, 'with_causation_id(', label);
  requireText(source, 'with_deadline(', label);
}

for (const [source, label] of [
  [orderStage, 'order stage'],
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
    orderCompletionOwner,
    'order completion owner',
    'CheckoutCompletionPort',
    'complete_checkout(',
  ],
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
    orderSettlementOwner,
    'order settlement owner',
    'CheckoutOrderPaymentSettlementPort',
    'mark_paid(',
  ],
]) {
  requireText(source, `trait ${port}`, label);
  requireText(source, operation, label);
  requireText(source, 'require_policy(PortCallPolicy::', label);
}

for (const [value, label] of [
  ['pub struct CheckoutOrderRecoveryAdapter', 'order recovery adapter'],
  ['recover_existing_checkout(', 'order recovery operation'],
  ['read_checkout_order(', 'order recovery read operation'],
  ['context.require_policy(PortCallPolicy::write())?;', 'order recovery write policy'],
  ['context.require_policy(PortCallPolicy::read())?;', 'order recovery read policy'],
]) requireText(orderRecoveryOwner, value, label);

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
  '✔ Checkout order, payment, fulfillment, order settlement, and pipeline recovery use sanitized owner boundaries',
);

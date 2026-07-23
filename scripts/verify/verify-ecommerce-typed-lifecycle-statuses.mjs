#!/usr/bin/env node

import { existsSync, readFileSync } from 'node:fs';

const root = new URL('../../', import.meta.url);
const read = (path) => readFileSync(new URL(path, root), 'utf8');
const failures = [];

const requireText = (source, value, label) => {
  if (!source.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (source, value, label) => {
  if (source.includes(value)) failures.push(`${label}: forbidden ${value}`);
};

const cart = read('crates/rustok-cart/src/dto/status.rs');
const cartLib = read('crates/rustok-cart/src/lib.rs');
const order = read('crates/rustok-order/src/status.rs');
const orderLib = read('crates/rustok-order/src/lib.rs');
const payment = read('crates/rustok-payment/src/dto/payment.rs');
const paymentStage = read('crates/rustok-commerce/src/services/checkout_payment_stages.rs');
const fulfillment = read('crates/rustok-fulfillment/src/status.rs');
const fulfillmentTypedExecution = read(
  'crates/rustok-fulfillment/src/checkout_execution_typed.rs',
);
const fulfillmentLib = read('crates/rustok-fulfillment/src/lib.rs');
const fulfillmentStage = read(
  'crates/rustok-commerce/src/services/checkout_fulfillment_stages.rs',
);

for (const [source, value, label] of [
  [cart, 'pub enum CartStatus', 'canonical cart status enum'],
  [cart, 'pub fn lifecycle_status(&self) -> CartResult<CartStatus>', 'cart typed accessor'],
  [cart, 'pub const fn can_begin_checkout(self) -> bool', 'cart begin predicate'],
  [cart, 'pub const fn can_complete_checkout(self) -> bool', 'cart completion predicate'],
  [cart, 'CartStatus::parse(self.status.as_str()).ok_or_else', 'cart unknown fail-closed mapping'],
  [order, 'pub enum OrderStatusKind', 'order status enum'],
  [order, 'pub enum OrderChangeStatusKind', 'order change status enum'],
  [order, 'pub enum OrderReturnStatusKind', 'order return status enum'],
  [order, 'pub fn status_kind(&self) -> OrderStatusKind', 'order typed accessor'],
  [order, '_ => Self::Unknown', 'order unknown mapping'],
  [orderLib, 'pub mod status;', 'order status module export'],
  [payment, 'pub enum PaymentCollectionStatusKind', 'payment collection status enum'],
  [payment, 'pub enum PaymentStatusKind', 'payment status enum'],
  [payment, 'pub enum RefundStatusKind', 'refund status enum'],
  [payment, 'pub fn status_kind(&self) -> PaymentCollectionStatusKind', 'payment typed accessor'],
  [payment, '_ => Self::Unknown', 'payment unknown mapping'],
  [paymentStage, 'PaymentCollectionStatusKind', 'mounted payment status import'],
  [paymentStage, 'authorized.status_kind().is_authorized_or_captured()', 'typed authorization result'],
  [paymentStage, 'captured.status_kind().is_captured()', 'typed capture result'],
  [paymentStage, 'collection.status_kind().is_captured()', 'typed captured replay'],
  [paymentStage, 'order.status_kind()', 'typed order admission'],
  [fulfillment, 'pub enum FulfillmentStatusKind', 'fulfillment status enum'],
  [fulfillment, 'pub fn status_kind(&self) -> FulfillmentStatusKind', 'fulfillment typed accessor'],
  [fulfillment, '_ => Self::Unknown', 'fulfillment unknown mapping'],
  [fulfillmentTypedExecution, 'match fulfillment.status_kind()', 'typed fulfillment dispatch'],
  [fulfillmentTypedExecution, 'FulfillmentStatusKind::Cancelled', 'cancelled reconciliation'],
  [fulfillmentTypedExecution, 'FulfillmentStatusKind::Unknown', 'unknown reconciliation'],
  [fulfillmentLib, 'mod checkout_execution_typed;', 'typed fulfillment factory module'],
  [
    fulfillmentLib,
    'TypedCheckoutFulfillmentExecutionPort, in_process_checkout_fulfillment_execution_port',
    'typed fulfillment root factory export',
  ],
  [fulfillmentLib, 'pub mod status;', 'fulfillment status module export'],
  [
    fulfillmentStage,
    'in_process_checkout_fulfillment_execution_port',
    'mounted fulfillment typed root factory',
  ],
]) {
  requireText(source, value, label);
}

for (const value of [
  'authorized.status.as_str()',
  'captured.status != "captured"',
  'collection.status != "captured"',
  'order.status.as_str()',
]) {
  forbidText(paymentStage, value, 'mounted payment stage raw lifecycle status');
}

forbidText(
  fulfillmentLib,
  'pub use checkout_execution::*;',
  'unsafe fulfillment root factory wildcard export',
);
forbidText(
  fulfillmentStage,
  'checkout_execution::in_process_checkout_fulfillment_execution_port',
  'mounted fulfillment legacy factory bypass',
);
forbidText(
  fulfillmentStage,
  'InProcessCheckoutFulfillmentExecutionPort',
  'mounted fulfillment direct adapter construction',
);

forbidText(cartLib, 'pub mod status;', 'duplicate cart status module export');
if (existsSync(new URL('crates/rustok-cart/src/status.rs', root))) {
  failures.push('duplicate cart lifecycle status file must not exist');
}

if (failures.length > 0) {
  console.error('Ecommerce typed lifecycle owner verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Ecommerce owners plus mounted payment and fulfillment paths use canonical typed lifecycle views',
);

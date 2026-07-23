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
const cartAtomic = read('crates/rustok-cart/src/atomic_checkout_port.rs');
const order = read('crates/rustok-order/src/status.rs');
const orderLib = read('crates/rustok-order/src/lib.rs');
const orderCompensation = read('crates/rustok-order/src/checkout_compensation.rs');
const orderRecovery = read('crates/rustok-order/src/checkout_order_recovery.rs');
const orderStage = read('crates/rustok-commerce/src/services/checkout_order_stages.rs');
const payment = read('crates/rustok-payment/src/dto/payment.rs');
const paymentPorts = read('crates/rustok-payment/src/ports.rs');
const paymentStage = read('crates/rustok-commerce/src/services/checkout_payment_stages.rs');
const fulfillment = read('crates/rustok-fulfillment/src/status.rs');
const fulfillmentTypedExecution = read(
  'crates/rustok-fulfillment/src/checkout_execution_typed.rs',
);
const fulfillmentLib = read('crates/rustok-fulfillment/src/lib.rs');
const fulfillmentStage = read(
  'crates/rustok-commerce/src/services/checkout_fulfillment_stages.rs',
);
const finalization = read('crates/rustok-commerce/src/services/checkout_finalization.rs');
const compensation = read(
  'crates/rustok-commerce/src/services/checkout_compensation_owner_ports.rs',
);

for (const [source, value, label] of [
  [cart, 'pub enum CartStatus', 'canonical cart status enum'],
  [cart, 'pub fn lifecycle_status(&self) -> CartResult<CartStatus>', 'cart typed accessor'],
  [cart, 'pub const fn can_begin_checkout(self) -> bool', 'cart begin predicate'],
  [cart, 'pub const fn can_complete_checkout(self) -> bool', 'cart completion predicate'],
  [cart, 'CartStatus::parse(self.status.as_str()).ok_or_else', 'cart unknown fail-closed mapping'],
  [cartAtomic, 'let status = cart_status(&cart)?;', 'atomic cart typed result'],
  [cartAtomic, 'let current_status = cart_status(&current)?;', 'atomic cart typed admission'],
  [cartAtomic, 'CartStatus::parse(from.as_str())', 'atomic transition race parse'],
  [cartAtomic, 'cart.lifecycle_status().map_err(cart_error_to_port_error)', 'atomic cart typed helper'],
  [order, 'pub enum OrderStatusKind', 'order status enum'],
  [order, 'pub enum OrderChangeStatusKind', 'order change status enum'],
  [order, 'pub enum OrderReturnStatusKind', 'order return status enum'],
  [order, 'pub fn status_kind(&self) -> OrderStatusKind', 'order typed accessor'],
  [order, '_ => Self::Unknown', 'order unknown mapping'],
  [orderLib, 'pub mod status;', 'order status module export'],
  [orderCompensation, 'pub fn status_kind(&self) -> OrderStatusKind', 'order snapshot typed accessor'],
  [orderRecovery, 'match order.status_kind()', 'typed order recovery dispatch'],
  [orderRecovery, 'OrderStatusKind::Pending', 'typed pending recovery'],
  [orderRecovery, 'OrderStatusKind::Cancelled', 'typed cancelled recovery'],
  [orderRecovery, 'OrderStatusKind::Unknown', 'typed unknown recovery'],
  [orderStage, 'OrderStatusKind', 'mounted order status import'],
  [orderStage, 'allowed_statuses: &[OrderStatusKind]', 'typed order projection policy'],
  [orderStage, 'allowed_statuses.contains(&order.status_kind())', 'typed order projection check'],
  [payment, 'pub enum PaymentCollectionStatusKind', 'payment collection status enum'],
  [payment, 'pub enum PaymentStatusKind', 'payment status enum'],
  [payment, 'pub enum RefundStatusKind', 'refund status enum'],
  [payment, 'pub fn status_kind(&self) -> PaymentCollectionStatusKind', 'payment typed accessor'],
  [payment, '_ => Self::Unknown', 'payment unknown mapping'],
  [paymentPorts, 'pub fn status_kind(&self) -> PaymentCollectionStatusKind', 'payment snapshot typed accessor'],
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
  [finalization, 'let cart = match cart_status(&current)?', 'typed cart finalization dispatch'],
  [finalization, 'PaymentCollectionStatusKind::Captured', 'typed finalization payment admission'],
  [finalization, 'state.order.status_kind()', 'typed finalization order admission'],
  [finalization, 'cart_status(cart)? != CartStatus::Completed', 'typed completed cart validation'],
  [compensation, 'snapshot.status_kind() != PaymentCollectionStatusKind::Cancelled', 'typed payment compensation result'],
  [compensation, 'snapshot.status_kind() != OrderStatusKind::Cancelled', 'typed order compensation result'],
  [compensation, 'match cart_status(&current)?', 'typed cart compensation dispatch'],
  [compensation, 'cart_status(&released)? != CartStatus::Active', 'typed cart release result'],
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

for (const value of [
  'match order.status.as_str()',
  '"pending" =>',
  '"confirmed" | "paid" | "shipped" | "delivered"',
]) {
  forbidText(orderRecovery, value, 'order checkout recovery raw lifecycle status');
}

for (const value of [
  'allowed_statuses: &[&str]',
  'allowed_statuses.contains(&order.status.as_str())',
  '&["confirmed"]',
  '&["confirmed", "paid", "shipped", "delivered"]',
]) {
  forbidText(orderStage, value, 'mounted order projection raw lifecycle status');
}

for (const value of [
  'cart.status.as_str()',
  'current.status.as_str()',
  'current.status == CartStatus::Active.as_str()',
]) {
  forbidText(cartAtomic, value, 'atomic cart raw lifecycle status');
}

for (const value of [
  'current.status == CartStatus::Completed.as_str()',
  'current.status.as_str()',
  'state.payment_collection.status != "captured"',
  'state.order.status.as_str()',
  'cart.status != CartStatus::Completed.as_str()',
]) {
  forbidText(finalization, value, 'checkout finalization raw lifecycle status');
}

for (const value of [
  'snapshot.status != "cancelled"',
  'current.status.as_str()',
  'released.status != CartStatus::Active.as_str()',
]) {
  forbidText(compensation, value, 'checkout compensation raw owner lifecycle status');
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
  '✔ Ecommerce owners plus mounted order, payment, fulfillment, cart finalization, and compensation paths use canonical typed lifecycle views',
);

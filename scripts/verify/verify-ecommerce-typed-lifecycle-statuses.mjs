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
const fulfillment = read('crates/rustok-fulfillment/src/status.rs');
const fulfillmentLib = read('crates/rustok-fulfillment/src/lib.rs');

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
  [fulfillment, 'pub enum FulfillmentStatusKind', 'fulfillment status enum'],
  [fulfillment, 'pub fn status_kind(&self) -> FulfillmentStatusKind', 'fulfillment typed accessor'],
  [fulfillment, '_ => Self::Unknown', 'fulfillment unknown mapping'],
  [fulfillmentLib, 'pub mod status;', 'fulfillment status module export'],
]) {
  requireText(source, value, label);
}

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
  '✔ Cart, order, payment, and fulfillment expose one canonical typed lifecycle view per owner',
);

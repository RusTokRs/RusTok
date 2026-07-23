#!/usr/bin/env node

import { readFileSync } from 'node:fs';

const root = new URL('../../', import.meta.url);
const read = (path) => readFileSync(new URL(path, root), 'utf8');
const failures = [];

const requireText = (source, value, label) => {
  if (!source.includes(value)) failures.push(`${label}: missing ${value}`);
};

const cart = read('crates/rustok-cart/src/status.rs');
const cartLib = read('crates/rustok-cart/src/lib.rs');
const order = read('crates/rustok-order/src/status.rs');
const orderLib = read('crates/rustok-order/src/lib.rs');
const payment = read('crates/rustok-payment/src/dto/payment.rs');
const fulfillment = read('crates/rustok-fulfillment/src/status.rs');
const fulfillmentLib = read('crates/rustok-fulfillment/src/lib.rs');

for (const [source, value, label] of [
  [cart, 'pub enum CartStatusKind', 'cart status enum'],
  [cart, 'pub fn status_kind(&self) -> CartStatusKind', 'cart typed accessor'],
  [cart, '_ => Self::Unknown', 'cart unknown mapping'],
  [cartLib, 'pub mod status;', 'cart status module export'],
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

if (failures.length > 0) {
  console.error('Ecommerce typed lifecycle owner verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Cart, order, payment, and fulfillment expose typed fail-closed lifecycle views',
);

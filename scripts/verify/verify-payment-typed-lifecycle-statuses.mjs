#!/usr/bin/env node

import { readFileSync } from 'node:fs';

const root = new URL('../../', import.meta.url);
const read = (path) => readFileSync(new URL(path, root), 'utf8');
const dto = read('crates/rustok-payment/src/dto/payment.rs');
const compensation = read('crates/rustok-payment/src/checkout_compensation.rs');
const failures = [];

const requireText = (source, value, label) => {
  if (!source.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (source, value, label) => {
  if (source.includes(value)) failures.push(`${label}: forbidden ${value}`);
};

for (const [value, label] of [
  ['pub enum PaymentCollectionStatusKind', 'collection status enum'],
  ['pub enum PaymentStatusKind', 'payment status enum'],
  ['pub enum RefundStatusKind', 'refund status enum'],
  ['pub fn status_kind(&self) -> PaymentCollectionStatusKind', 'collection typed accessor'],
  ['pub fn status_kind(&self) -> PaymentStatusKind', 'payment typed accessor'],
  ['pub fn status_kind(&self) -> RefundStatusKind', 'refund typed accessor'],
  ['"authorized" => Self::Authorized', 'authorized mapping'],
  ['"captured" => Self::Captured', 'captured mapping'],
  ['"refunded" => Self::Refunded', 'refunded mapping'],
  ['_ => Self::Unknown', 'unknown fail-closed mapping'],
  ['pub const fn can_authorize(self) -> bool', 'authorize predicate'],
  ['pub const fn can_capture(self) -> bool', 'capture predicate'],
]) {
  requireText(dto, value, label);
}

for (const [value, label] of [
  ['PaymentCollectionStatusKind', 'compensation typed status import'],
  ['match collection.status_kind()', 'compensation typed dispatch'],
  ['current.status_kind() == PaymentCollectionStatusKind::Cancelled', 'cancel race adoption'],
  ['collection.status_kind() == PaymentCollectionStatusKind::Authorized', 'provider cancel predicate'],
  ['PaymentCollectionStatusKind::Unknown', 'unknown compensation reconciliation'],
]) {
  requireText(compensation, value, label);
}

for (const value of [
  'match collection.status.as_str()',
  'current.status == "cancelled"',
  'collection.status == "authorized"',
]) {
  forbidText(compensation, value, 'checkout compensation raw collection status');
}

if (failures.length > 0) {
  console.error('Payment typed lifecycle status verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log('✔ Payment DTOs and checkout compensation use typed fail-closed lifecycle status views');

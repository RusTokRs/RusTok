#!/usr/bin/env node

import { readFileSync } from 'node:fs';

const root = new URL('../../', import.meta.url);
const dto = readFileSync(new URL('crates/rustok-payment/src/dto/payment.rs', root), 'utf8');
const failures = [];

const requireText = (value, label) => {
  if (!dto.includes(value)) failures.push(`${label}: missing ${value}`);
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
  requireText(value, label);
}

if (failures.length > 0) {
  console.error('Payment typed lifecycle status verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log('✔ Payment DTOs expose typed fail-closed lifecycle status views');

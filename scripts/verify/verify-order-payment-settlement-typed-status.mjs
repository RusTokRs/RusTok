#!/usr/bin/env node

import { readFileSync } from 'node:fs';

const root = new URL('../../', import.meta.url);
const source = readFileSync(
  new URL('crates/rustok-order/src/checkout_payment_settlement.rs', root),
  'utf8',
);
const failures = [];

const requireText = (value, label) => {
  if (!source.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (value, label) => {
  if (source.includes(value)) failures.push(`${label}: forbidden ${value}`);
};

requireText('OrderStatusKind', 'typed status import');
requireText('match current.status_kind()', 'typed settlement dispatch');
requireText('OrderStatusKind::Confirmed', 'confirmed transition');
requireText(
  'OrderStatusKind::Paid | OrderStatusKind::Shipped | OrderStatusKind::Delivered',
  'idempotent settled states',
);
requireText('OrderStatusKind::Unknown', 'unknown fail-closed state');
forbidText('match current.status.as_str()', 'raw status dispatch');
forbidText('"paid" | "shipped" | "delivered"', 'raw settled-state literals');

if (failures.length > 0) {
  console.error('Order payment settlement typed-status verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log('✔ Order payment settlement dispatches through OrderStatusKind');

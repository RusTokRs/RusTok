#!/usr/bin/env node

import { readFileSync } from 'node:fs';

const root = new URL('../../', import.meta.url);
const source = readFileSync(
  new URL('crates/rustok-order/src/checkout_compensation.rs', root),
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
requireText('match order.status_kind()', 'typed compensation dispatch');
requireText(
  'OrderStatusKind::Pending | OrderStatusKind::Confirmed',
  'cancellable states',
);
requireText(
  'OrderStatusKind::Paid | OrderStatusKind::Shipped | OrderStatusKind::Delivered',
  'manual financial reconciliation states',
);
requireText('OrderStatusKind::Unknown', 'unknown manual reconciliation state');
requireText(
  'current.status_kind() == OrderStatusKind::Cancelled',
  'typed concurrent cancellation adoption',
);
forbidText('match order.status.as_str()', 'raw status dispatch');
forbidText('current.status == "cancelled"', 'raw cancellation adoption');

if (failures.length > 0) {
  console.error('Order compensation typed-status verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log('✔ Order checkout compensation dispatches through OrderStatusKind');

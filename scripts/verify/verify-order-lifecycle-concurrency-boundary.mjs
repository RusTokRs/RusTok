#!/usr/bin/env node

import { readFileSync } from 'node:fs';

const root = new URL('../../', import.meta.url);
const source = readFileSync(
  new URL('crates/modules/rustok-order/src/services/order.rs', root),
  'utf8',
);

const failures = [];
const requireText = (value, label) => {
  if (!source.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (value, label) => {
  if (source.includes(value)) failures.push(`${label}: forbidden ${value}`);
};

for (const marker of [
  'async fn find_order_for_update_in_tx(',
  'async fn find_order_change_for_update_in_tx(',
  'async fn find_order_return_for_update_in_tx(',
  'DatabaseBackend::Postgres | DatabaseBackend::MySql',
  'DatabaseBackend::Sqlite',
  'query.lock_exclusive().one(txn).await?',
]) requireText(marker, 'order concurrency boundary');

for (const marker of [
  'find_order_for_update_in_tx(&txn, tenant_id, order_id).await?',
  'find_order_change_for_update_in_tx(&txn, tenant_id, change_id).await?',
  'find_order_return_for_update_in_tx(&txn, tenant_id, return_id).await?',
  'let txn = self.db.begin().await?',
  'active.update(&txn).await?',
  'txn.commit().await?',
]) requireText(marker, 'order concurrency transaction');

forbidText(
  'self.load_order_model_in_tx(&txn, tenant_id, order_id)',
  'order lifecycle must not use an unlocked order read',
);
forbidText(
  'active.update(&self.db)',
  'order state mutation must remain inside the lock transaction',
);

if (failures.length) {
  for (const failure of failures) console.error(failure);
  process.exit(1);
}
console.log('order lifecycle concurrency boundary: ok');

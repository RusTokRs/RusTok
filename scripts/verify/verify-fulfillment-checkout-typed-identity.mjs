#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(path.resolve(configuredRoot) + path.sep)
  : new URL('../../', import.meta.url);
const read = (relativePath) => readFileSync(new URL(relativePath, root), 'utf8');
const failures = [];

const requireText = (content, value, label) => {
  if (!content.includes(value)) failures.push(label + ': missing ' + value);
};
const forbidText = (content, value, label) => {
  if (content.includes(value)) failures.push(label + ': forbidden ' + value);
};

const checkout = read('crates/modules/rustok-fulfillment/src/checkout_execution.rs');
const service = read('crates/modules/rustok-fulfillment/src/services/fulfillment.rs');
const entity = read('crates/modules/rustok-fulfillment/src/entities/fulfillment.rs');
const migration = read(
  'crates/modules/rustok-fulfillment/src/migrations/m20260925_000119_type_checkout_fulfillment_identity.rs',
);
const contract = JSON.parse(
  read('crates/modules/rustok-fulfillment/contracts/fulfillment-checkout-execution-v1.json'),
);

for (const [content, values, label] of [
  [
    entity,
    [
      'pub checkout_operation_id: Option<Uuid>,',
      'pub checkout_fulfillment_index: Option<i64>,',
      'pub checkout_plan_hash: Option<String>,',
    ],
    'typed fulfillment entity',
  ],
  [
    service,
    [
      'pub(crate) async fn create_checkout_fulfillment(',
      'pub(crate) async fn find_checkout_fulfillment(',
      'pub(crate) async fn list_checkout_fulfillments(',
      'Column::CheckoutOperationId.eq(checkout_operation_id)',
      'Column::CheckoutFulfillmentIndex',
      'checkout_plan_hash.clone()',
    ],
    'typed owner service',
  ],
  [
    checkout,
    [
      'create_checkout_fulfillment(',
      'find_checkout_fulfillment(',
      'list_checkout_fulfillments(',
      '"find_checkout_fulfillment_before_create"',
      '"adopt_checkout_fulfillment_after_create_error"',
      '"list_checkout_fulfillments_for_read"',
    ],
    'checkout typed identity path',
  ],
  [
    migration,
    [
      'ADD COLUMN checkout_operation_id',
      'ADD COLUMN checkout_fulfillment_index',
      'ADD COLUMN checkout_plan_hash',
      'ON fulfillments (tenant_id, checkout_operation_id, checkout_fulfillment_index)',
      'metadata = metadata',
      "metadata #>> '{checkout,fulfillment_key}'",
      'DROP COLUMN checkout_plan_hash',
      'DROP COLUMN checkout_fulfillment_index',
      'DROP COLUMN checkout_operation_id',
    ],
    'typed identity migration',
  ],
]) {
  for (const value of values) requireText(content, value, label);
}

for (const value of [
  'fn fulfillment_key(',
  'fn fulfillment_index(',
  'list_by_order(tenant_id, order_id)',
  'get("checkout")',
]) {
  forbidText(checkout, value, 'checkout legacy identity implementation');
}

requireText(migration, 'CREATE UNIQUE INDEX ux_fulfillments_checkout_identity', 'typed identity uniqueness');
requireText(
  migration,
  "length(btrim(metadata #>> '{checkout,fulfillment_index}')) <= 10",
  'PostgreSQL legacy fulfillment index cast is length-bounded',
);
requireText(
  migration,
  "THEN (btrim(metadata #>> '{checkout,fulfillment_index}'))::bigint = checkout_fulfillment_index",
  'PostgreSQL legacy fulfillment index comparison uses a guarded cast',
);
requireText(contract, '"typed_identity_migration_required": false', 'contract migration completion');
requireText(contract, '"identity_guard": "scripts/verify/verify-fulfillment-checkout-typed-identity.mjs"', 'contract identity guard');
requireText(
  migration,
  'UPDATE fulfillment_items AS fi\n            SET metadata = jsonb_set(',
  'PostgreSQL down migration restores fulfillment item checkout identity',
);
requireText(
  migration,
  "UPDATE fulfillment_items\n            SET metadata = json_set(",
  'SQLite down migration restores fulfillment item checkout identity',
);
requireText(
  migration,
  "UPDATE fulfillment_items AS fi\n            JOIN fulfillments AS f",
  'MySQL down migration restores fulfillment item checkout identity',
);

const uniquenessCount = (migration.match(/ux_fulfillments_checkout_identity/g) || []).length;
if (uniquenessCount < 2) {
  failures.push('typed identity migration must create and restore the canonical unique index');
}

if (failures.length > 0) {
  console.error('Fulfillment typed checkout identity verification failed:');
  for (const failure of failures) console.error('✗ ' + failure);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Fulfillment checkout identity is persisted and recovered through typed owner fields with metadata identity removed from the runtime path',
);

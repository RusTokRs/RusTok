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
const expect = (condition, label) => {
  if (!condition) failures.push(`${label}: contract assertion failed`);
};

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
      'ensure_checkout_fulfillment_set(',
      'list_checkout_fulfillments(',
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


// Only the *retired* metadata identity accessors are forbidden. `checkout.cart_line_item_id`
// is still a live metadata field (written by checkout execution and Commerce adoption), so a
// blanket ban on reading the `checkout` object forbade supported code.
for (const value of [
  'fn fulfillment_key(',
  'fn fulfillment_index(',
  'list_by_order(tenant_id, order_id)',
  'checkout.get("fulfillment_key")',
  'checkout.get("fulfillment_index")',
  'checkout.get("operation_id")',
  'checkout.get("order_plan_hash")',
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
// The contract is JSON, so the assertions read the parsed document instead of its
// serialized text. A text marker here matched the file's formatting, not its meaning,
// and the previous version passed the parsed object to `requireText`, which threw.
expect(
  contract?.compatibility?.typed_identity_migration_required === false,
  'contract migration completion',
);
expect(
  contract?.verification?.identity_guard ===
    'scripts/verify/verify-fulfillment-checkout-typed-identity.mjs',
  'contract identity guard',
);
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
requireText(
  migration,
  "btrim(fi.metadata #>> '{checkout,operation_id}') = f.checkout_operation_id::text",
  'PostgreSQL item cleanup requires owner identity match',
);
requireText(
  migration,
  // The comparison is case-insensitive on purpose, so the pinned fragment is the
  // parent-bound predicate itself rather than one exact spelling of it.
  "json_extract(fulfillment_items.metadata, '$.checkout.operation_id')",
  'SQLite item cleanup is parent-bound',
);
requireText(
  migration,
  "lower(trim(json_extract(fulfillment_items.metadata, '$.checkout.operation_id')))",
  'SQLite item cleanup compares the parent operation case-insensitively',
);
requireText(
  migration,
  'DROP TRIGGER IF EXISTS fulfillments_checkout_identity_guard_insert;',
  'MySQL typed cutover removes the legacy INSERT guard',
);
requireText(
  migration,
  "OR trim(COALESCE(json_extract(NEW.metadata, '$.checkout.operation_id'), '')) = ''",
  'SQLite rollback restores legacy identity-pair validation on UPDATE',
);
requireText(
  migration,
  "JSON_UNQUOTE(JSON_EXTRACT(NEW.metadata, '$.checkout.operation_id'))",
  'MySQL rollback restores legacy identity-pair validation',
);
requireText(
  migration,
  'CREATE TRIGGER fulfillments_checkout_identity_guard_insert',
  'MySQL rollback restores the legacy INSERT guard',
);
requireText(
  migration,
  "= LOWER(f.checkout_operation_id)",
  'MySQL item cleanup requires owner identity match',
);

const uniquenessCount = (migration.match(/ux_fulfillments_checkout_identity/g) || []).length;
if (uniquenessCount < 2) {
  failures.push('typed identity migration must create and restore the canonical unique index');
}

requireText(service, 'pub(crate) async fn ensure_checkout_fulfillment_set(', 'aggregate checkout ensure service');
requireText(service, '.lock_exclusive()', 'operation anchor is locked during checkout ensure');
requireText(service, 'index outside the requested immutable plan', 'aggregate checkout ensure rejects extra indices');
forbidText(checkout, 'create_checkout_fulfillment(', 'checkout execution no longer performs per-index create/adopt loops');

if (failures.length > 0) {
  console.error('Fulfillment typed checkout identity verification failed:');
  for (const failure of failures) console.error('✗ ' + failure);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Fulfillment checkout identity is persisted and recovered through typed owner fields with metadata identity removed from the runtime path',
);

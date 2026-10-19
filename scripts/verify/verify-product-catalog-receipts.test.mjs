#!/usr/bin/env node
import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import {
  sourcePaths,
  verifyCatalogReceipts
} from './verify-product-catalog-receipts.mjs';
const sources = Object.fromEntries(
  Object.entries(sourcePaths).map(([key, path]) => [
    key,
    readFileSync(new URL(`../../${path}`, import.meta.url), 'utf8')
  ])
);
test('current catalog receipt wiring satisfies the contract', () => {
  assert.deepEqual(verifyCatalogReceipts(sources), []);
});
for (const [name, key, before, after] of [
  [
    'unreceipted command',
    'port',
    'execute_catalog_command(',
    'unreceipted_command('
  ],
  [
    'missing no-op result',
    'commands',
    'record_product_operation_result(&result);',
    'forget_result();'
  ],
  ['pool product projection', 'projection', '.one(conn)', '.one(&self.db)'],
  ['pool tag relations', 'tags', '.all(conn)', '.all(&self.db)'],
  [
    'pool taxonomy names',
    'tags',
    '.resolve_term_names_in(conn,',
    '.resolve_term_names('
  ],
  [
    'pool taxonomy translations',
    'taxonomy',
    'load_translations_map_in(conn,',
    'load_translations_map('
  ],
  [
    'missing success replay',
    'port',
    'Admission::Replay(value)',
    'Admission::Ignored(value)'
  ],
  [
    'missing terminal replay',
    'port',
    'Admission::ReplayError(error)',
    'Admission::IgnoredError(error)'
  ],
  [
    'missing failure finalization',
    'port',
    'fail_product_operation_receipt(service.database(), lease, &error)',
    'forget_failure()'
  ],
  [
    'success completion outside transaction',
    'transaction',
    'idempotency::complete(&transaction',
    'idempotency::complete(&database'
  ]
]) {
  test(`${name} is rejected`, () => {
    assert.ok(
      sources[key].includes(before),
      'mutation must change a production marker'
    );
    const mutated = { ...sources, [key]: sources[key].replace(before, after) };
    assert.ok(verifyCatalogReceipts(mutated).length > 0);
  });
}

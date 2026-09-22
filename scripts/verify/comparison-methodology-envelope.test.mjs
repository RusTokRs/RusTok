#!/usr/bin/env node

import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import {
  existsSync,
  mkdtempSync,
  rmSync,
  writeFileSync,
} from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { test } from 'node:test';

import {
  comparableDatabaseFields,
  comparisonMethodologyKeys,
  databaseSettingsSource,
  requireComparisonDatabaseSettingsMethodology,
} from './index-storage-database-settings-contract.mjs';

const prepareScript = path.resolve('scripts/verify/prepare-index-storage-decision.mjs');
const renderScript = path.resolve('scripts/verify/render-index-storage-adr.mjs');
const envelopeError = /comparison methodology must contain exactly the canonical methodology fields/u;

const methodology = () => ({
  source_oracle: 'normalized idx_bench_source workload result digests',
  result_digest: 'ordered_length_prefixed_json_v1',
  evidence_validation: 'fail closed on report shape, metrics, plans, effects, ordering, digest semantics, and cardinalities',
  first_run: 'first EXPLAIN ANALYZE repetition',
  warm_run: 'median after the first repetition; not a guaranteed OS cold-cache comparison',
  automatic_winner_selection: false,
  comparable_database_fields: [...comparableDatabaseFields],
  database_settings_source: databaseSettingsSource,
});

const requireMethodology = (value) => requireComparisonDatabaseSettingsMethodology(
  { methodology: value },
  (message) => { throw new Error(message); },
);

const writeJson = (filename, value) => writeFileSync(
  filename,
  `${JSON.stringify(value, null, 2)}\n`,
  'utf8',
);

const run = (script, args) => spawnSync(process.execPath, [script, ...args], {
  encoding: 'utf8',
});

const withFixture = (callback) => {
  const root = mkdtempSync(path.join(tmpdir(), 'rustok-index-methodology-envelope-'));
  try {
    callback(root);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
};

test('accepts exactly the canonical comparison methodology envelope', () => {
  const value = methodology();
  assert.deepEqual(Object.keys(value), [...comparisonMethodologyKeys]);
  assert.equal(requireMethodology(value), value);
});

test('rejects a missing comparison methodology field', () => {
  const value = methodology();
  delete value.first_run;
  assert.throws(() => requireMethodology(value), envelopeError);
});

test('rejects a renamed comparison methodology field', () => {
  const value = methodology();
  value.cold_run = value.first_run;
  delete value.first_run;
  assert.throws(() => requireMethodology(value), envelopeError);
});

test('rejects an additional comparison methodology field', () => {
  const value = methodology();
  value.unreviewed_note = 'must not become decision input';
  assert.throws(() => requireMethodology(value), envelopeError);
});

test('decision preparation rejects methodology drift before publishing a draft', () => {
  withFixture((root) => {
    const comparisonPath = path.join(root, 'comparison.json');
    const decisionPath = path.join(root, 'decision.json');
    const value = methodology();
    value.unreviewed_note = 'must not become decision input';
    writeJson(comparisonPath, { methodology: value });

    const result = run(prepareScript, [
      '--comparison', comparisonPath,
      '--selected', 'typed_eav',
      '--owner', 'Index maintainers',
      '--date', '2026-07-26',
      '--output', decisionPath,
    ]);

    assert.notEqual(result.status, 0);
    assert.match(result.stderr, envelopeError);
    assert.equal(existsSync(decisionPath), false);
  });
});

test('direct ADR rendering rejects methodology drift before publishing output', () => {
  withFixture((root) => {
    const comparisonPath = path.join(root, 'comparison.json');
    const decisionPath = path.join(root, 'decision.json');
    const outputPath = path.join(root, 'adr.md');
    const value = methodology();
    value.unreviewed_note = 'must not reach the ADR';
    writeJson(comparisonPath, { methodology: value });
    writeJson(decisionPath, {});

    const result = run(renderScript, [
      '--comparison', comparisonPath,
      '--decision', decisionPath,
      '--output', outputPath,
    ]);

    assert.notEqual(result.status, 0);
    assert.match(result.stderr, envelopeError);
    assert.equal(existsSync(outputPath), false);
  });
});

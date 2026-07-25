#!/usr/bin/env node

import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import {
  existsSync,
  mkdtempSync,
  readFileSync,
  readdirSync,
  rmSync,
  writeFileSync,
} from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { test } from 'node:test';

import {
  comparableDatabaseFields,
  databaseSettingsSource,
} from './index-storage-database-settings-contract.mjs';

const finalizer = path.resolve('scripts/verify/finalize-index-storage-adr.mjs');

const writeJson = (filename, value) => {
  writeFileSync(filename, `${JSON.stringify(value, null, 2)}\n`, 'utf8');
};

const comparison = () => ({
  methodology: {
    source_oracle: 'normalized idx_bench_source workload result digests',
    result_digest: 'ordered_length_prefixed_json_v1',
    evidence_validation: 'fail closed on report shape, metrics, plans, effects, ordering, digest semantics, and cardinalities',
    first_run: 'first EXPLAIN ANALYZE repetition',
    warm_run: 'median after the first repetition; not a guaranteed OS cold-cache comparison',
    automatic_winner_selection: false,
    comparable_database_fields: [...comparableDatabaseFields],
    database_settings_source: databaseSettingsSource,
  },
});

const decision = (overrides = {}) => ({
  status: 'accepted',
  decision_date: '2026-07-25',
  owner: 'Index maintainers',
  comparison_commit: '0123456789abcdef0123456789abcdef01234567',
  comparison_sha256: '0'.repeat(64),
  selected_prototype: 'typed_eav',
  selection_rationale: 'Measured evidence supports the selected prototype.',
  rejection_rationales: {
    jsonb: 'JSONB was not selected.',
    hot_projection: 'Hot projection was not selected.',
  },
  operational_tradeoffs: 'Monitor relation growth, WAL, and VACUUM behavior.',
  migration_strategy: 'Backfill and verify parity before cutover.',
  rollback_strategy: 'Retain the previous path until cutover verification completes.',
  ...overrides,
});

const runFinalizerArgs = (args) => spawnSync(process.execPath, [finalizer, ...args], {
  encoding: 'utf8',
});

const stagingEntries = (root) => readdirSync(root)
  .filter((entry) => entry.startsWith('adr.md.tmp-'));

const runFinalizer = (root, decisionValue, staleOutput = null) => {
  const comparisonPath = path.join(root, 'comparison.json');
  const decisionPath = path.join(root, 'decision.json');
  const outputPath = path.join(root, 'adr.md');
  writeJson(comparisonPath, comparison());
  writeJson(decisionPath, decisionValue);
  if (staleOutput !== null) writeFileSync(outputPath, staleOutput, 'utf8');
  return {
    comparisonPath,
    decisionPath,
    outputPath,
    result: runFinalizerArgs([
      '--comparison', comparisonPath,
      '--decision', decisionPath,
      '--output', outputPath,
    ]),
  };
};

const withFixture = (prefix, callback) => {
  const root = mkdtempSync(path.join(tmpdir(), prefix));
  try {
    callback(root);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
};

test('finalizer accepts help only as the sole argument', () => {
  const result = runFinalizerArgs(['--help']);
  assert.equal(result.status, 0);
  assert.match(result.stdout, /^Usage: node scripts\/verify\/finalize-index-storage-adr\.mjs /u);
  assert.equal(result.stderr, '');
});

test('finalizer rejects mixed help without changing an existing ADR', () => {
  withFixture('rustok-index-finalizer-help-', (root) => {
    const outputPath = path.join(root, 'adr.md');
    const original = Buffer.from('accepted ADR\n', 'utf8');
    writeFileSync(outputPath, original);
    const result = runFinalizerArgs(['--help', '--output', outputPath]);
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /--help\/-h must be the only argument/u);
    assert.doesNotMatch(result.stderr, /missing comparison/u);
    assert.equal(result.stdout, '');
    assert.deepEqual(readFileSync(outputPath), original);
  });
});

test('finalizer rejects unknown options without changing an existing ADR', () => {
  withFixture('rustok-index-finalizer-unknown-', (root) => {
    const outputPath = path.join(root, 'adr.md');
    const original = Buffer.from('accepted ADR\n', 'utf8');
    writeFileSync(outputPath, original);
    const result = runFinalizerArgs([
      '--comparison', 'missing-comparison.json',
      '--decision', 'missing-decision.json',
      '--output', outputPath,
      '--format', 'markdown',
    ]);
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /unknown argument: --format/u);
    assert.doesNotMatch(result.stderr, /missing comparison/u);
    assert.equal(result.stdout, '');
    assert.deepEqual(readFileSync(outputPath), original);
  });
});

test('finalizer input collision preserves the comparison bytes', () => {
  withFixture('rustok-index-finalizer-collision-', (root) => {
    const comparisonPath = path.join(root, 'comparison.json');
    const decisionPath = path.join(root, 'decision.json');
    writeJson(comparisonPath, comparison());
    writeJson(decisionPath, decision());
    const original = readFileSync(comparisonPath);
    const result = runFinalizerArgs([
      '--comparison', comparisonPath,
      '--decision', decisionPath,
      '--output', comparisonPath,
    ]);
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /--output must not overwrite the comparison input/u);
    assert.deepEqual(readFileSync(comparisonPath), original);
  });
});

test('real finalization attempts revoke stale ADR before evidence access', () => {
  withFixture('rustok-index-finalizer-missing-', (root) => {
    const outputPath = path.join(root, 'adr.md');
    writeFileSync(outputPath, 'stale ADR\n', 'utf8');
    const result = runFinalizerArgs([
      '--comparison', path.join(root, 'missing-comparison.json'),
      '--decision', path.join(root, 'missing-decision.json'),
      '--output', outputPath,
    ]);
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /missing comparison/u);
    assert.equal(existsSync(outputPath), false);
    assert.deepEqual(stagingEntries(root), []);
  });
});

test('finalizer rejects an impossible decision date without leaving stale output', () => {
  withFixture('rustok-index-decision-date-', (root) => {
    const { outputPath, result } = runFinalizer(root, decision({
      decision_date: '2026-02-31',
    }), 'stale ADR\n');
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /decision\.decision_date must be a real ISO calendar date/u);
    assert.equal(existsSync(outputPath), false);
    assert.deepEqual(stagingEntries(root), []);
  });
});

test('finalizer rejects a proposed decision without leaving stale output', () => {
  withFixture('rustok-index-decision-status-', (root) => {
    const { outputPath, result } = runFinalizer(
      root,
      decision({ status: 'proposed' }),
      'stale ADR\n',
    );
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /decision\.status must be accepted before ADR finalization/u);
    assert.equal(existsSync(outputPath), false);
    assert.deepEqual(stagingEntries(root), []);
  });
});

test('renderer failure leaves neither stale ADR nor staged output', () => {
  withFixture('rustok-index-finalizer-render-', (root) => {
    const { outputPath, result } = runFinalizer(root, decision(), 'stale ADR\n');
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /strict ADR renderer failed/u);
    assert.equal(existsSync(outputPath), false);
    assert.deepEqual(stagingEntries(root), []);
  });
});

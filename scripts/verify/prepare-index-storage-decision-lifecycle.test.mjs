#!/usr/bin/env node

import { spawnSync } from 'node:child_process';
import {
  existsSync,
  mkdtempSync,
  readdirSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import assert from 'node:assert/strict';
import { test } from 'node:test';

import {
  comparableDatabaseFields,
  databaseSettingsSource,
} from './index-storage-database-settings-contract.mjs';

const script = path.resolve('scripts/verify/prepare-index-storage-decision.mjs');
const commit = '0123456789abcdef0123456789abcdef01234567';
const requiredDecisionFlags = [
  'required_scales_present',
  'same_packet_contract_version',
  'same_result_digest_contract',
  'same_repository',
  'same_commit',
  'same_postgres_image',
  'same_repetitions',
  'same_churn_cycles',
  'same_database_settings',
  'same_dataset_shape',
  'same_source_oracle_shape',
  'same_report_shape',
  'same_mutation_effect_contract',
];

const validComparison = () => ({
  generated_at: '2026-07-24T12:00:00Z',
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
  decision_ready: true,
  decision_contract: Object.fromEntries(requiredDecisionFlags.map((field) => [field, true])),
  scales: [
    { scale: '100k', provenance: { commit } },
    { scale: '1m', provenance: { commit } },
  ],
});

const serializeJson = (value) => `${JSON.stringify(value, null, 2)}\n`;
const writeJson = (filename, value) => writeFileSync(filename, serializeJson(value), 'utf8');
const invoke = (args) => spawnSync(process.execPath, [script, ...args], { encoding: 'utf8' });

const withFixture = (callback) => {
  const root = mkdtempSync(path.join(tmpdir(), 'rustok-index-decision-preparation-'));
  try {
    callback(root);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
};

const paths = (root) => ({
  comparisonPath: path.join(root, 'comparison.json'),
  decisionPath: path.join(root, 'decision.json'),
});

const baseArgs = (comparisonPath, decisionPath) => [
  '--comparison', comparisonPath,
  '--selected', 'typed_eav',
  '--owner', 'Index maintainers',
  '--date', '2026-07-24',
  '--output', decisionPath,
];

const stagingEntries = (root) => readdirSync(root)
  .filter((entry) => entry.startsWith('.decision.json.tmp-'));

test('help is successful only as the sole argument', () => {
  const result = invoke(['--help']);
  assert.equal(result.status, 0, result.stderr || result.stdout);
  assert.match(result.stdout, /Usage:/u);

  const mixed = invoke(['--help', '--output', 'decision.json']);
  assert.notEqual(mixed.status, 0);
  assert.match(mixed.stderr, /help must be the only argument/u);
  assert.equal(mixed.stdout, '');
});

test('unknown forced arguments preserve an existing decision', () => {
  withFixture((root) => {
    const { comparisonPath, decisionPath } = paths(root);
    const original = Buffer.from('{"reviewed":true}\n', 'utf8');
    writeFileSync(decisionPath, original);

    const result = invoke([
      ...baseArgs(comparisonPath, decisionPath),
      '--force',
      '--format', 'json',
    ]);
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /unknown or incomplete argument: --format/u);
    assert.deepEqual(readFileSync(decisionPath), original);
    assert.deepEqual(stagingEntries(root), []);
  });
});

test('duplicate arguments preserve an existing decision', () => {
  withFixture((root) => {
    const { comparisonPath, decisionPath } = paths(root);
    const original = Buffer.from('{"reviewed":true}\n', 'utf8');
    writeFileSync(decisionPath, original);

    const duplicateOutput = invoke([
      ...baseArgs(comparisonPath, decisionPath),
      '--output', path.join(root, 'other.json'),
      '--force',
    ]);
    assert.notEqual(duplicateOutput.status, 0);
    assert.match(duplicateOutput.stderr, /--output was provided more than once/u);
    assert.deepEqual(readFileSync(decisionPath), original);

    const duplicateForce = invoke([
      ...baseArgs(comparisonPath, decisionPath),
      '--force',
      '--force',
    ]);
    assert.notEqual(duplicateForce.status, 0);
    assert.match(duplicateForce.stderr, /--force was provided more than once/u);
    assert.deepEqual(readFileSync(decisionPath), original);
    assert.deepEqual(stagingEntries(root), []);
  });
});

test('output collision is non-destructive', () => {
  withFixture((root) => {
    const { comparisonPath } = paths(root);
    writeJson(comparisonPath, validComparison());
    const original = readFileSync(comparisonPath);

    const result = invoke(baseArgs(comparisonPath, comparisonPath));
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /--output must not overwrite the comparison input/u);
    assert.deepEqual(readFileSync(comparisonPath), original);
    assert.deepEqual(stagingEntries(root), []);
  });
});

test('non-forced preparation preserves an existing decision', () => {
  withFixture((root) => {
    const { comparisonPath, decisionPath } = paths(root);
    writeJson(comparisonPath, validComparison());
    const original = Buffer.from('{"reviewed":true}\n', 'utf8');
    writeFileSync(decisionPath, original);

    const result = invoke(baseArgs(comparisonPath, decisionPath));
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /refusing to overwrite existing decision without --force/u);
    assert.deepEqual(readFileSync(decisionPath), original);
    assert.deepEqual(stagingEntries(root), []);
  });
});

test('forced preparation revokes stale output before comparison validation', () => {
  withFixture((root) => {
    const { comparisonPath, decisionPath } = paths(root);
    const comparison = validComparison();
    comparison.decision_ready = false;
    writeJson(comparisonPath, comparison);
    writeFileSync(decisionPath, '{"stale":true}\n', 'utf8');

    const result = invoke([...baseArgs(comparisonPath, decisionPath), '--force']);
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /comparison is not decision-ready/u);
    assert.equal(existsSync(decisionPath), false);
    assert.deepEqual(stagingEntries(root), []);
  });
});

test('forced preparation revokes stale output before comparison access', () => {
  withFixture((root) => {
    const { comparisonPath, decisionPath } = paths(root);
    writeFileSync(decisionPath, '{"stale":true}\n', 'utf8');

    const result = invoke([...baseArgs(comparisonPath, decisionPath), '--force']);
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /missing comparison/u);
    assert.equal(existsSync(decisionPath), false);
    assert.deepEqual(stagingEntries(root), []);
  });
});

test('successful forced preparation publishes one fresh draft atomically', () => {
  withFixture((root) => {
    const { comparisonPath, decisionPath } = paths(root);
    writeJson(comparisonPath, validComparison());
    writeFileSync(decisionPath, '{"stale":true}\n', 'utf8');

    const result = invoke([
      ...baseArgs(comparisonPath, decisionPath),
      '--selected', 'jsonb',
      '--date', '2026-07-25',
      '--force',
    ]);
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /--selected was provided more than once/u);
    assert.equal(existsSync(decisionPath), true);

    const replacement = invoke([
      '--comparison', comparisonPath,
      '--selected', 'jsonb',
      '--owner', 'Index maintainers',
      '--date', '2026-07-25',
      '--output', decisionPath,
      '--force',
    ]);
    assert.equal(replacement.status, 0, replacement.stderr || replacement.stdout);
    const decision = JSON.parse(readFileSync(decisionPath, 'utf8'));
    assert.equal(decision.status, 'proposed');
    assert.equal(decision.selected_prototype, 'jsonb');
    assert.equal(decision.decision_date, '2026-07-25');
    assert.equal(decision.comparison_commit, commit);
    assert.deepEqual(Object.keys(decision.rejection_rationales), ['typed_eav', 'hot_projection']);
    assert.deepEqual(stagingEntries(root), []);
  });
});

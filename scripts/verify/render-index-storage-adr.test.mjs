#!/usr/bin/env node

import { createHash } from 'node:crypto';
import { spawnSync } from 'node:child_process';
import {
  existsSync,
  mkdtempSync,
  mkdirSync,
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

const script = path.resolve('scripts/verify/render-index-storage-adr.mjs');
const commit = '0123456789abcdef0123456789abcdef01234567';
const prototypes = ['jsonb', 'typed_eav', 'hot_projection'];
const readWorkloads = ['status_equality', 'price_range_sort'];
const mutationWorkloads = ['update_product_batch', 'delete_product_batch'];

const decisionFlags = {
  required_scales_present: true,
  same_packet_contract_version: true,
  same_result_digest_contract: true,
  same_repository: true,
  same_commit: true,
  same_postgres_image: true,
  same_repetitions: true,
  same_churn_cycles: true,
  same_database_settings: true,
  same_dataset_shape: true,
  same_source_oracle_shape: true,
  same_report_shape: true,
  same_mutation_effect_contract: true,
};

const readPrototype = (prototype, scaleFactor) => ({
  prototype,
  schema: prototype === 'typed_eav'
    ? 'idx_bench_eav'
    : `idx_bench_${prototype === 'hot_projection' ? 'hot' : 'jsonb'}`,
  load_ms: 10 * scaleFactor,
  schema_bytes: 1_000 * scaleFactor,
  workloads: readWorkloads.map((name, index) => ({
    name,
    warm_median_execution_ms: (index + 1) * scaleFactor,
    plan_shape_variants: 1,
  })),
});

const mutationPrototype = (prototype, scaleFactor) => ({
  prototype,
  workloads: mutationWorkloads.map((name, index) => ({
    name,
    median_execution_ms: (index + 2) * scaleFactor,
    median_maximum_node_wal_bytes: (index + 3) * 1_024 * scaleFactor,
  })),
});

const maintenancePrototype = (prototype, scaleFactor) => ({
  prototype,
  after_churn: {
    field_rows: prototype === 'typed_eav' ? 1_400_160 * scaleFactor : null,
  },
  churn_growth_percent: 5 * scaleFactor,
  vacuum_duration_ms: 20 * scaleFactor,
});

const scale = (name, scaleFactor) => ({
  scale: name,
  provenance: {
    packet_contract_version: 2,
    result_digest_contract: 'ordered_length_prefixed_json_v1',
    repository: 'RusTokRs/RusTok',
    commit,
    postgres_image: 'postgres:16',
  },
  read: prototypes.map((prototype) => readPrototype(prototype, scaleFactor)),
  mutation: prototypes.map((prototype) => mutationPrototype(prototype, scaleFactor)),
  maintenance: prototypes.map((prototype) => maintenancePrototype(prototype, scaleFactor)),
});

const ratios = {
  prototypes: prototypes.map((prototype) => ({
    prototype,
    schema_bytes_ratio_1m_to_100k: 10,
    read_workloads: readWorkloads.map((name) => ({
      name,
      warm_execution_ratio_1m_to_100k: 10,
    })),
    mutation_workloads: mutationWorkloads.map((name) => ({
      name,
      execution_ratio_1m_to_100k: 10,
      wal_bytes_ratio_1m_to_100k: 10,
    })),
  })),
};

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
  decision_contract: { ...decisionFlags },
  scales: [scale('100k', 1), scale('1m', 10)],
  cross_scale_ratios: ratios,
});

const serializeJson = (value) => `${JSON.stringify(value, null, 2)}\n`;
const sha256Json = (value) => createHash('sha256').update(serializeJson(value)).digest('hex');

const validDecision = (comparison = validComparison()) => ({
  status: 'proposed',
  decision_date: '2026-07-24',
  owner: 'Index maintainers',
  comparison_commit: commit,
  comparison_sha256: sha256Json(comparison),
  selected_prototype: 'typed_eav',
  selection_rationale: 'Typed EAV provides the selected balance of query behavior and schema evolution.',
  rejection_rationales: {
    jsonb: 'JSONB was rejected because the measured and operational trade-offs were less suitable.',
    hot_projection: 'Hot projection was rejected because its migration and schema-expansion cost was higher.',
  },
  operational_tradeoffs: 'Operate field indexes explicitly and monitor relation growth, WAL, and VACUUM behavior.',
  migration_strategy: 'Introduce the selected tables behind the persistence port, backfill, verify, then cut over reads.',
  rollback_strategy: 'Keep the previous persistence path readable until verification and switch the port back on failure.',
});

const writeJson = (filename, value) => {
  mkdirSync(path.dirname(filename), { recursive: true });
  writeFileSync(filename, serializeJson(value));
};

const withFixture = (callback) => {
  const root = mkdtempSync(path.join(tmpdir(), 'rustok-index-adr-'));
  try {
    callback(root);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
};

const invoke = (args) => spawnSync(process.execPath, [script, ...args], { encoding: 'utf8' });

const run = (root, comparison, decision, extraArgs = []) => {
  const comparisonPath = path.join(root, 'comparison.json');
  const decisionPath = path.join(root, 'decision.json');
  const outputPath = path.join(root, 'adr.md');
  writeJson(comparisonPath, comparison);
  writeJson(decisionPath, decision);
  const result = invoke([
    '--comparison', comparisonPath,
    '--decision', decisionPath,
    '--output', outputPath,
    ...extraArgs,
  ]);
  return { result, comparisonPath, decisionPath, outputPath };
};

const stagingEntries = (root) => readdirSync(root)
  .filter((entry) => entry.startsWith('.adr.md.tmp-'));

test('renders a manual same-commit storage ADR', () => {
  withFixture((root) => {
    const comparison = validComparison();
    const { result, outputPath } = run(root, comparison, validDecision(comparison));
    assert.equal(result.status, 0, result.stderr || result.stdout);
    const markdown = readFileSync(outputPath, 'utf8');
    assert.match(markdown, /Use \*\*typed_eav\*\*/u);
    assert.match(markdown, /ordered_length_prefixed_json_v1/u);
    assert.match(markdown, /Comparison SHA-256/u);
    assert.match(markdown, /## Rejected alternatives/u);
    assert.match(markdown, /renderer does not infer or rank a winning prototype/u);
    assert.deepEqual(stagingEntries(root), []);
  });
});

test('rejects core-only comparison without observed database-settings methodology', () => {
  withFixture((root) => {
    const comparison = validComparison();
    comparison.methodology.comparable_database_fields = ['server_version_num'];
    const { result, outputPath } = run(root, comparison, validDecision(comparison));
    assert.notEqual(result.status, 0);
    assert.match(
      result.stderr,
      /comparable_database_fields must exactly match the canonical PostgreSQL database-settings contract/u,
    );
    assert.equal(existsSync(outputPath), false);
  });
});

test('rejects evidence that is not decision-ready', () => {
  withFixture((root) => {
    const comparison = validComparison();
    comparison.decision_ready = false;
    const { result, outputPath } = run(root, comparison, validDecision());
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /comparison is not decision-ready/u);
    assert.equal(existsSync(outputPath), false);
  });
});

test('rejects a decision tied to another commit', () => {
  withFixture((root) => {
    const comparison = validComparison();
    const decision = validDecision(comparison);
    decision.comparison_commit = 'ffffffffffffffffffffffffffffffffffffffff';
    const { result } = run(root, comparison, decision);
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /must match the evidence comparison commit/u);
  });
});

test('rejects comparison bytes changed after the decision', () => {
  withFixture((root) => {
    const comparison = validComparison();
    const decision = validDecision(comparison);
    comparison.generated_at = '2026-07-24T12:00:01Z';
    const { result } = run(root, comparison, decision);
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /must match the exact comparison\.json bytes/u);
  });
});

test('requires rationale for every rejected alternative', () => {
  withFixture((root) => {
    const comparison = validComparison();
    const decision = validDecision(comparison);
    delete decision.rejection_rationales.hot_projection;
    const { result } = run(root, comparison, decision);
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /must contain exactly jsonb, hot_projection/u);
  });
});

test('rejects an unsatisfied evidence decision flag', () => {
  withFixture((root) => {
    const comparison = validComparison();
    const decision = validDecision(comparison);
    comparison.decision_contract.same_result_digest_contract = false;
    const { result } = run(root, comparison, decision);
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /same_result_digest_contract is not satisfied/u);
  });
});

test('never overwrites the comparison input', () => {
  withFixture((root) => {
    const comparison = validComparison();
    const decision = validDecision(comparison);
    const comparisonPath = path.join(root, 'comparison.json');
    const decisionPath = path.join(root, 'decision.json');
    writeJson(comparisonPath, comparison);
    writeJson(decisionPath, decision);
    const original = readFileSync(comparisonPath);
    const result = invoke([
      '--comparison', comparisonPath,
      '--decision', decisionPath,
      '--output', comparisonPath,
    ]);
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /--output must not overwrite the comparison input/u);
    assert.deepEqual(readFileSync(comparisonPath), original);
  });
});

test('never overwrites the decision input', () => {
  withFixture((root) => {
    const comparison = validComparison();
    const decision = validDecision(comparison);
    const comparisonPath = path.join(root, 'comparison.json');
    const decisionPath = path.join(root, 'decision.json');
    writeJson(comparisonPath, comparison);
    writeJson(decisionPath, decision);
    const original = readFileSync(decisionPath);
    const result = invoke([
      '--comparison', comparisonPath,
      '--decision', decisionPath,
      '--output', decisionPath,
    ]);
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /--output must not overwrite the decision input/u);
    assert.deepEqual(readFileSync(decisionPath), original);
  });
});

test('help is successful only as the sole argument', () => {
  const result = invoke(['--help']);
  assert.equal(result.status, 0, result.stderr || result.stdout);
  assert.match(result.stdout, /Usage:/u);
});

test('mixed help preserves an existing ADR output', () => {
  withFixture((root) => {
    const outputPath = path.join(root, 'adr.md');
    const original = Buffer.from('reviewed ADR\n', 'utf8');
    writeFileSync(outputPath, original);
    const result = invoke(['--help', '--output', outputPath]);
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /help must be the only argument/u);
    assert.deepEqual(readFileSync(outputPath), original);
  });
});

test('unknown and duplicate options fail before changing output', () => {
  withFixture((root) => {
    const outputPath = path.join(root, 'adr.md');
    const original = Buffer.from('reviewed ADR\n', 'utf8');
    writeFileSync(outputPath, original);
    const unknown = invoke([
      '--comparison', 'missing-comparison.json',
      '--decision', 'missing-decision.json',
      '--output', outputPath,
      '--format', 'markdown',
    ]);
    assert.notEqual(unknown.status, 0);
    assert.match(unknown.stderr, /unknown or incomplete argument: --format/u);
    assert.deepEqual(readFileSync(outputPath), original);

    const duplicate = invoke([
      '--comparison', 'missing-comparison.json',
      '--decision', 'missing-decision.json',
      '--output', outputPath,
      '--output', path.join(root, 'other.md'),
    ]);
    assert.notEqual(duplicate.status, 0);
    assert.match(duplicate.stderr, /--output was provided more than once/u);
    assert.deepEqual(readFileSync(outputPath), original);
  });
});

test('real render attempts revoke stale ADR output before core validation', () => {
  withFixture((root) => {
    const outputPath = path.join(root, 'adr.md');
    writeFileSync(outputPath, 'stale ADR\n', 'utf8');
    const result = invoke([
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

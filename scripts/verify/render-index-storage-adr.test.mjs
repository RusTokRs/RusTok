#!/usr/bin/env node

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';

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
  schema: prototype === 'typed_eav' ? 'idx_bench_eav' : `idx_bench_${prototype === 'hot_projection' ? 'hot' : 'jsonb'}`,
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
  methodology: { automatic_winner_selection: false },
  decision_ready: true,
  decision_contract: { ...decisionFlags },
  scales: [scale('100k', 1), scale('1m', 10)],
  cross_scale_ratios: ratios,
});

const validDecision = () => ({
  status: 'proposed',
  decision_date: '2026-07-24',
  owner: 'Index maintainers',
  comparison_commit: commit,
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
  writeFileSync(filename, `${JSON.stringify(value, null, 2)}\n`);
};

const withFixture = (callback) => {
  const root = mkdtempSync(path.join(tmpdir(), 'rustok-index-adr-'));
  try {
    callback(root);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
};

const run = (root, comparison, decision) => {
  const comparisonPath = path.join(root, 'comparison.json');
  const decisionPath = path.join(root, 'decision.json');
  const outputPath = path.join(root, 'adr.md');
  writeJson(comparisonPath, comparison);
  writeJson(decisionPath, decision);
  const result = spawnSync('node', [
    script,
    '--comparison', comparisonPath,
    '--decision', decisionPath,
    '--output', outputPath,
  ], { encoding: 'utf8' });
  return { result, outputPath };
};

test('renders a manual same-commit storage ADR', () => {
  withFixture((root) => {
    const { result, outputPath } = run(root, validComparison(), validDecision());
    assert.equal(result.status, 0, result.stderr || result.stdout);
    const markdown = readFileSync(outputPath, 'utf8');
    assert.match(markdown, /Use \*\*typed_eav\*\*/u);
    assert.match(markdown, /ordered_length_prefixed_json_v1/u);
    assert.match(markdown, /## Rejected alternatives/u);
    assert.match(markdown, /renderer does not infer or rank a winning prototype/u);
  });
});

test('rejects evidence that is not decision-ready', () => {
  withFixture((root) => {
    const comparison = validComparison();
    comparison.decision_ready = false;
    const { result } = run(root, comparison, validDecision());
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /comparison is not decision-ready/u);
  });
});

test('rejects a decision tied to another commit', () => {
  withFixture((root) => {
    const decision = validDecision();
    decision.comparison_commit = 'ffffffffffffffffffffffffffffffffffffffff';
    const { result } = run(root, validComparison(), decision);
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /must match the evidence comparison commit/u);
  });
});

test('requires rationale for every rejected alternative', () => {
  withFixture((root) => {
    const decision = validDecision();
    delete decision.rejection_rationales.hot_projection;
    const { result } = run(root, validComparison(), decision);
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /must contain exactly jsonb, hot_projection/u);
  });
});

test('rejects an unsatisfied evidence decision flag', () => {
  withFixture((root) => {
    const comparison = validComparison();
    comparison.decision_contract.same_result_digest_contract = false;
    const { result } = run(root, comparison, validDecision());
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /same_result_digest_contract is not satisfied/u);
  });
});

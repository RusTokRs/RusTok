#!/usr/bin/env node

import { createHash } from 'node:crypto';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import assert from 'node:assert/strict';
import { test } from 'node:test';

const prepareScript = path.resolve('scripts/verify/prepare-index-storage-decision.mjs');
const finalizeScript = path.resolve('scripts/verify/finalize-index-storage-adr.mjs');
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

const scale = (name, factor) => ({
  scale: name,
  provenance: {
    packet_contract_version: 2,
    result_digest_contract: 'ordered_length_prefixed_json_v1',
    repository: 'RusTokRs/RusTok',
    commit,
    postgres_image: 'postgres:16',
  },
  read: prototypes.map((prototype) => ({
    prototype,
    schema_bytes: 1_024 * factor,
    workloads: readWorkloads.map((workload, index) => ({
      name: workload,
      warm_median_execution_ms: (index + 1) * factor,
      plan_shape_variants: 1,
    })),
  })),
  mutation: prototypes.map((prototype) => ({
    prototype,
    workloads: mutationWorkloads.map((workload, index) => ({
      name: workload,
      median_execution_ms: (index + 2) * factor,
      median_maximum_node_wal_bytes: (index + 3) * 1_024 * factor,
    })),
  })),
  maintenance: prototypes.map((prototype) => ({
    prototype,
    after_churn: {
      field_rows: prototype === 'typed_eav' ? 1_400_160 * factor : null,
    },
    churn_growth_percent: 5 * factor,
    vacuum_duration_ms: 20 * factor,
  })),
});

const comparison = () => ({
  generated_at: '2026-07-24T12:00:00Z',
  methodology: { automatic_winner_selection: false },
  decision_ready: true,
  decision_contract: { ...decisionFlags },
  scales: [scale('100k', 1), scale('1m', 10)],
  cross_scale_ratios: {
    prototypes: prototypes.map((prototype) => ({
      prototype,
      schema_bytes_ratio_1m_to_100k: 10,
      read_workloads: readWorkloads.map((workload) => ({
        name: workload,
        warm_execution_ratio_1m_to_100k: 10,
      })),
      mutation_workloads: mutationWorkloads.map((workload) => ({
        name: workload,
        execution_ratio_1m_to_100k: 10,
        wal_bytes_ratio_1m_to_100k: 10,
      })),
    })),
  },
});

const writeJson = (filename, value) => {
  const text = `${JSON.stringify(value, null, 2)}\n`;
  writeFileSync(filename, text, 'utf8');
  return Buffer.from(text, 'utf8');
};
const sha256 = (bytes) => createHash('sha256').update(bytes).digest('hex');
const run = (script, args) => spawnSync(process.execPath, [script, ...args], { encoding: 'utf8' });

const withFixture = (callback) => {
  const root = mkdtempSync(path.join(tmpdir(), 'rustok-index-decision-'));
  try {
    callback(root);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
};

const prepare = (root) => {
  const comparisonPath = path.join(root, 'comparison.json');
  const decisionPath = path.join(root, 'decision.json');
  const comparisonBytes = writeJson(comparisonPath, comparison());
  const result = run(prepareScript, [
    '--comparison', comparisonPath,
    '--selected', 'typed_eav',
    '--owner', 'Index maintainers',
    '--date', '2026-07-24',
    '--output', decisionPath,
  ]);
  return { result, comparisonPath, decisionPath, comparisonBytes };
};

const completeDecision = (decision) => ({
  ...decision,
  selection_rationale: 'Typed EAV provides the selected balance of measured query behavior and schema evolution.',
  rejection_rationales: {
    jsonb: 'JSONB was not selected because its measured and operational trade-offs were less suitable.',
    hot_projection: 'Hot projection was not selected because its migration and schema-expansion cost was higher.',
  },
  operational_tradeoffs: 'Operate field indexes explicitly and monitor relation growth, WAL, and VACUUM behavior.',
  migration_strategy: 'Create the selected tables, backfill, verify parity, and cut over the persistence port.',
  rollback_strategy: 'Keep the previous persistence path readable until verification and switch the port back on failure.',
});

test('prepares an exact-comparison-bound manual decision draft', () => {
  withFixture((root) => {
    const { result, decisionPath, comparisonBytes } = prepare(root);
    assert.equal(result.status, 0, result.stderr || result.stdout);
    const decision = JSON.parse(readFileSync(decisionPath, 'utf8'));
    assert.equal(Object.hasOwn(decision, '$schema'), false);
    assert.equal(decision.comparison_commit, commit);
    assert.equal(decision.comparison_sha256, sha256(comparisonBytes));
    assert.equal(decision.selected_prototype, 'typed_eav');
    assert.deepEqual(Object.keys(decision.rejection_rationales), ['jsonb', 'hot_projection']);
    assert.match(decision.selection_rationale, /^TODO\(index-storage-decision\):/u);
  });
});

test('refuses to overwrite an existing decision without force', () => {
  withFixture((root) => {
    const fixture = prepare(root);
    assert.equal(fixture.result.status, 0, fixture.result.stderr || fixture.result.stdout);
    const result = run(prepareScript, [
      '--comparison', fixture.comparisonPath,
      '--selected', 'typed_eav',
      '--owner', 'Index maintainers',
      '--date', '2026-07-24',
      '--output', fixture.decisionPath,
    ]);
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /refusing to overwrite existing decision without --force/u);
  });
});

test('never overwrites the comparison input even with force', () => {
  withFixture((root) => {
    const comparisonPath = path.join(root, 'comparison.json');
    const original = writeJson(comparisonPath, comparison());
    const result = run(prepareScript, [
      '--comparison', comparisonPath,
      '--selected', 'typed_eav',
      '--owner', 'Index maintainers',
      '--date', '2026-07-24',
      '--output', comparisonPath,
      '--force',
    ]);
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /--output must not overwrite the comparison input/u);
    assert.deepEqual(readFileSync(comparisonPath), original);
  });
});

test('rejects an unedited prepared decision', () => {
  withFixture((root) => {
    const fixture = prepare(root);
    assert.equal(fixture.result.status, 0, fixture.result.stderr || fixture.result.stdout);
    const result = run(finalizeScript, [
      '--comparison', fixture.comparisonPath,
      '--decision', fixture.decisionPath,
      '--output', path.join(root, 'adr.md'),
    ]);
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /still contains a preparation placeholder/u);
  });
});

test('rejects unsupported fields in the decision envelope', () => {
  withFixture((root) => {
    const fixture = prepare(root);
    assert.equal(fixture.result.status, 0, fixture.result.stderr || fixture.result.stdout);
    const decision = completeDecision(JSON.parse(readFileSync(fixture.decisionPath, 'utf8')));
    decision.unreviewed_note = 'This field must not be silently ignored.';
    writeJson(fixture.decisionPath, decision);
    const result = run(finalizeScript, [
      '--comparison', fixture.comparisonPath,
      '--decision', fixture.decisionPath,
      '--output', path.join(root, 'adr.md'),
    ]);
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /decision contains unsupported field unreviewed_note/u);
  });
});

test('finalizes an ADR bound to exact comparison and decision bytes', () => {
  withFixture((root) => {
    const fixture = prepare(root);
    assert.equal(fixture.result.status, 0, fixture.result.stderr || fixture.result.stdout);
    const decision = completeDecision(JSON.parse(readFileSync(fixture.decisionPath, 'utf8')));
    const decisionBytes = writeJson(fixture.decisionPath, decision);
    const outputPath = path.join(root, 'adr.md');
    const result = run(finalizeScript, [
      '--comparison', fixture.comparisonPath,
      '--decision', fixture.decisionPath,
      '--output', outputPath,
    ]);
    assert.equal(result.status, 0, result.stderr || result.stdout);
    const markdown = readFileSync(outputPath, 'utf8');
    assert.match(markdown, new RegExp(`Comparison SHA-256: \\`${sha256(fixture.comparisonBytes)}\\``, 'u'));
    assert.match(markdown, new RegExp(`Decision SHA-256: \\`${sha256(decisionBytes)}\\``, 'u'));
    assert.match(markdown, /Use \*\*typed_eav\*\*/u);
  });
});

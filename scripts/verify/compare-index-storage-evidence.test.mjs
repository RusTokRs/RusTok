#!/usr/bin/env node

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';

const scriptPath = path.resolve('scripts/verify/compare-index-storage-evidence.mjs');
const prototypes = ['jsonb', 'typed_eav', 'hot_projection'];
const readWorkloads = ['status_equality', 'price_range_sort'];
const mutationWorkloads = ['update_product_batch', 'delete_product_batch'];

const scaleValues = {
  '100k': { serialized: 'rows100k', debug: 'Rows100k', entities: 300_080, links: 600_000 },
  '1m': { serialized: 'rows1m', debug: 'Rows1m', entities: 3_000_160, links: 6_000_000 },
};

const repetition = (seed = 1) => ({
  planning_time_ms: seed,
  execution_time_ms: seed + 1,
  shared_hit_blocks: seed + 2,
  shared_read_blocks: seed + 3,
  temporary_read_blocks: 0,
  temporary_written_blocks: 0,
  maximum_node_wal_records: seed + 4,
  maximum_node_wal_fpi: seed + 5,
  maximum_node_wal_bytes: seed + 6,
  plan: [{ Plan: { 'Node Type': 'Index Scan', 'Index Name': 'fixture_index' } }],
});

const repetitions = () => [repetition(1), repetition(2), repetition(3)];

const tableStats = () => [{
  estimated_live_tuples: 10,
  estimated_dead_tuples: 0,
  tuples_inserted: 10,
  tuples_updated: 1,
  tuples_deleted: 0,
  hot_updates: 0,
}];

const snapshot = (values, schemaBytes) => ({
  schema_bytes: schemaBytes,
  entity_rows: values.entities,
  link_rows: values.links,
  table_stats: tableStats(),
});

function writeJson(file, value) {
  mkdirSync(path.dirname(file), { recursive: true });
  writeFileSync(file, `${JSON.stringify(value, null, 2)}\n`);
}

function writePacket(root, scale, overrides = {}) {
  const values = scaleValues[scale];
  const directory = path.join(root, scale);
  const readNames = overrides.readWorkloads ?? readWorkloads;
  const mutationNames = overrides.mutationWorkloads ?? mutationWorkloads;
  const prototypeNames = overrides.prototypes ?? prototypes;

  const read = {
    database: {
      version: 'PostgreSQL 16 fixture',
      server_version_num: '160000',
      shared_buffers: '128MB',
      effective_cache_size: '4GB',
      work_mem: '4MB',
      random_page_cost: '4',
      jit: 'off',
      ...overrides.database,
    },
    dataset: { scale: values.serialized },
    source_load_ms: 10,
    source_entity_rows: values.entities,
    source_link_rows: values.links,
    prototypes: prototypeNames.map((prototype, prototypeIndex) => ({
      prototype,
      schema: `idx_bench_${prototype}`,
      load_ms: 20 + prototypeIndex,
      schema_bytes: 1000 + prototypeIndex,
      entity_rows: values.entities,
      link_rows: values.links,
      workloads: readNames.map((name, workloadIndex) => ({
        name,
        result_rows: 10,
        result_digest: `${name}-digest`,
        repetitions: repetitions().map((item) => ({
          ...item,
          execution_time_ms: item.execution_time_ms + workloadIndex,
        })),
      })),
    })),
  };

  const mutation = {
    dataset_scale: values.debug,
    prototypes: prototypeNames.map((prototype) => ({
      prototype,
      schema: `idx_bench_${prototype}`,
      workloads: mutationNames.map((name) => ({
        name,
        affected_entities: 1000,
        affected_links: name === 'delete_product_batch' ? 2000 : null,
        repetitions: repetitions(),
      })),
    })),
  };

  const maintenance = {
    dataset_scale: values.serialized,
    prototypes: prototypeNames.map((prototype, index) => ({
      prototype,
      schema: `idx_bench_${prototype}`,
      baseline: snapshot(values, 1000 + index),
      after_churn: snapshot(values, 1100 + index),
      after_vacuum: snapshot(values, 1110 + index),
      vacuum_duration_ms: 25 + index,
    })),
  };

  const provenance = {
    repository: 'RusTokRs/RusTok',
    commit: '0123456789abcdef0123456789abcdef01234567',
    ref: 'refs/heads/main',
    run_id: scale === '100k' ? '100' : '101',
    run_attempt: '1',
    postgres_image: 'postgres:16',
    runner_os: 'Linux',
    runner_arch: 'X64',
    scale,
    repetitions: 3,
    churn_cycles: 5,
    ...overrides.provenance,
  };

  writeJson(path.join(directory, 'read-report.json'), read);
  writeJson(path.join(directory, 'mutation-report.json'), mutation);
  writeJson(path.join(directory, 'maintenance-report.json'), maintenance);
  writeJson(path.join(directory, 'provenance.json'), provenance);
  return directory;
}

function runComparator(inputs, output) {
  const args = [scriptPath];
  for (const input of inputs) args.push('--input', input);
  args.push('--output', output);
  return spawnSync('node', args, { encoding: 'utf8' });
}

function withFixture(callback) {
  const root = mkdtempSync(path.join(tmpdir(), 'rustok-index-comparison-'));
  try {
    callback(root);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
}

test('same-commit 100k and 1m packets are decision-ready', () => {
  withFixture((root) => {
    const lower = writePacket(root, '100k');
    const upper = writePacket(root, '1m');
    const output = path.join(root, 'comparison');
    const result = runComparator([lower, upper], output);

    assert.equal(result.status, 0, result.stderr || result.stdout);
    const report = JSON.parse(readFileSync(path.join(output, 'comparison.json'), 'utf8'));
    assert.equal(report.decision_ready, true);
    assert.deepEqual(report.decision_contract, {
      required_scales_present: true,
      same_repository: true,
      same_commit: true,
      same_postgres_image: true,
      same_repetitions: true,
      same_churn_cycles: true,
      same_database_settings: true,
      same_report_shape: true,
    });
  });
});

test('one scale remains non-decision-ready', () => {
  withFixture((root) => {
    const lower = writePacket(root, '100k');
    const output = path.join(root, 'comparison');
    const result = runComparator([lower], output);

    assert.equal(result.status, 0, result.stderr || result.stdout);
    const report = JSON.parse(readFileSync(path.join(output, 'comparison.json'), 'utf8'));
    assert.equal(report.decision_ready, false);
    assert.equal(report.decision_contract.required_scales_present, false);
  });
});

for (const [label, field, value, pattern] of [
  ['repository', 'repository', 'OtherOrg/OtherRepo', /repository mismatch/],
  ['commit', 'commit', 'ffffffffffffffffffffffffffffffffffffffff', /commit mismatch/],
  ['PostgreSQL image', 'postgres_image', 'postgres:17', /PostgreSQL image mismatch/],
  ['repetitions', 'repetitions', 4, /repetitions mismatch/],
  ['churn cycles', 'churn_cycles', 6, /churn_cycles mismatch/],
]) {
  test(`rejects cross-scale ${label} mismatch`, () => {
    withFixture((root) => {
      const lower = writePacket(root, '100k');
      const upper = writePacket(root, '1m', { provenance: { [field]: value } });
      const result = runComparator([lower, upper], path.join(root, 'comparison'));

      assert.notEqual(result.status, 0, 'expected comparator to fail closed');
      assert.match(result.stderr, pattern);
    });
  });
}

test('rejects cross-scale PostgreSQL setting mismatch', () => {
  withFixture((root) => {
    const lower = writePacket(root, '100k');
    const upper = writePacket(root, '1m', { database: { work_mem: '8MB' } });
    const result = runComparator([lower, upper], path.join(root, 'comparison'));

    assert.notEqual(result.status, 0, 'expected comparator to fail closed');
    assert.match(result.stderr, /database setting work_mem mismatch/);
  });
});

test('rejects cross-scale workload-shape mismatch', () => {
  withFixture((root) => {
    const lower = writePacket(root, '100k');
    const upper = writePacket(root, '1m', {
      readWorkloads: [...readWorkloads, 'unexpected_workload'],
    });
    const result = runComparator([lower, upper], path.join(root, 'comparison'));

    assert.notEqual(result.status, 0, 'expected comparator to fail closed');
    assert.match(result.stderr, /read workload ordering mismatch/);
  });
});

test('rejects missing required provenance', () => {
  withFixture((root) => {
    const lower = writePacket(root, '100k');
    const upper = writePacket(root, '1m', { provenance: { commit: null } });
    const result = runComparator([lower, upper], path.join(root, 'comparison'));

    assert.notEqual(result.status, 0, 'expected comparator to fail closed');
    assert.match(result.stderr, /1m provenance is missing commit/);
  });
});

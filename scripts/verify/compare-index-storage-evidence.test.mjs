#!/usr/bin/env node

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';

const scriptPath = path.resolve('scripts/verify/compare-index-storage-evidence.mjs');
const generatedAt = '2026-07-24T12:00:00Z';
const locales = ['en-US', 'ru-RU'];
const prototypes = [
  { prototype: 'jsonb', schema: 'idx_bench_jsonb', relations: ['entity', 'link'] },
  { prototype: 'typed_eav', schema: 'idx_bench_eav', relations: ['entity', 'field_value', 'link'] },
  {
    prototype: 'hot_projection',
    schema: 'idx_bench_hot',
    relations: ['link', 'product', 'sales_channel', 'variant'],
  },
];
const readWorkloads = [
  'status_equality',
  'price_range_sort',
  'multi_value_tag',
  'two_hop_channel_filter',
  'keyset_page',
  'exact_count',
];
const mutationWorkloads = ['update_product_batch', 'delete_product_batch'];
const scaleValues = {
  '100k': {
    serialized: 'rows100k',
    debug: 'Rows100k',
    tenants: 10,
    productsPerTenant: 5_000,
    products: 100_000,
    entities: 300_080,
    fields: 1_400_160,
    links: 600_000,
    resultRows: 10,
  },
  '1m': {
    serialized: 'rows1m',
    debug: 'Rows1m',
    tenants: 20,
    productsPerTenant: 25_000,
    products: 1_000_000,
    entities: 3_000_160,
    fields: 14_000_320,
    links: 6_000_000,
    resultRows: 100,
  },
};

const digest = (workloadIndex, scale) => {
  const offset = scale === '100k' ? 1 : 8;
  return ((workloadIndex + offset) % 16).toString(16).repeat(32);
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

const tableStats = (relations) => relations.map((relation, index) => ({
  relation,
  estimated_live_tuples: 10 + index,
  estimated_dead_tuples: index,
  tuples_inserted: 10 + index,
  tuples_updated: 1,
  tuples_deleted: 0,
  hot_updates: 0,
  vacuum_count: 0,
  autovacuum_count: 0,
  analyze_count: 1,
  autoanalyze_count: 0,
}));
const snapshot = (values, prototype, schemaBytes, fieldRowsOverride) => ({
  captured_at: generatedAt,
  schema_bytes: schemaBytes,
  entity_rows: values.entities,
  field_rows: fieldRowsOverride ?? (prototype.prototype === 'typed_eav' ? values.fields : null),
  link_rows: values.links,
  table_stats: tableStats(prototype.relations),
});

const writeJson = (file, value) => {
  mkdirSync(path.dirname(file), { recursive: true });
  writeFileSync(file, `${JSON.stringify(value, null, 2)}\n`);
};
const mutationEffect = (prototype, workload, values) => ({
  affected_entities: 1_000,
  affected_fields: prototype === 'typed_eav'
    ? (workload === 'update_product_batch' ? 2_000 : 8_000)
    : null,
  affected_links: workload === 'delete_product_batch' ? 2_000 : null,
});

function writePacket(root, scale, overrides = {}) {
  const values = scaleValues[scale];
  const directory = path.join(root, scale);
  mkdirSync(directory, { recursive: true });
  const sourceNames = overrides.sourceWorkloads ?? readWorkloads;
  const readRepetitions = overrides.readRepetitions ?? 3;
  const mutationRepetitions = overrides.mutationRepetitions ?? 3;

  const source_workloads = sourceNames.map((name) => {
    const index = readWorkloads.indexOf(name);
    return {
      name,
      sql: `SELECT ${index + 1} FROM idx_bench_source.product`,
      result_rows: values.resultRows,
      result_digest: digest(index, scale),
    };
  });
  const read = {
    generated_at: generatedAt,
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
    dataset: {
      scale: values.serialized,
      tenants: values.tenants,
      products_per_tenant: values.productsPerTenant,
      locales,
      variants_per_product: 2,
      channels_per_tenant: 8,
      sales_channels_per_variant: 2,
      ...overrides.dataset,
    },
    source_load_ms: 10,
    source_entity_rows: values.entities,
    source_link_rows: values.links,
    source_workloads: overrides.omitSourceWorkloads ? undefined : source_workloads,
    prototypes: prototypes.map((prototype, prototypeIndex) => ({
      prototype: prototype.prototype,
      schema: prototype.schema,
      load_ms: 20 + prototypeIndex,
      schema_bytes: 1_000 + prototypeIndex,
      entity_rows: values.entities,
      link_rows: values.links,
      workloads: readWorkloads.map((name, workloadIndex) => {
        const evidence = repetitions().slice(0, readRepetitions);
        const override = overrides.readEvidence?.[prototype.prototype]?.[name];
        if (override) Object.assign(evidence[0], override);
        return {
          name,
          sql: `SELECT ${workloadIndex + 1} FROM ${prototype.schema}.entity`,
          result_rows: values.resultRows,
          result_digest: overrides.candidateDigest?.[prototype.prototype]?.[name]
            ?? digest(workloadIndex, scale),
          repetitions: evidence,
        };
      }),
    })),
  };

  const mutation = {
    generated_at: generatedAt,
    dataset_scale: values.debug,
    repetitions: 3,
    prototypes: prototypes.map((prototype) => ({
      prototype: prototype.prototype,
      schema: prototype.schema,
      workloads: mutationWorkloads.map((name) => {
        const evidence = repetitions().slice(0, mutationRepetitions);
        const override = overrides.mutationEvidence?.[prototype.prototype]?.[name];
        if (override) Object.assign(evidence[0], override);
        return {
          name,
          sql: 'SELECT affected_fields, expected_fields, affected_links, expected_links',
          ...mutationEffect(prototype.prototype, name, values),
          repetitions: evidence,
        };
      }),
    })),
  };

  const maintenance = {
    generated_at: generatedAt,
    dataset_scale: values.serialized,
    cycles: overrides.maintenanceCycles ?? 5,
    prototypes: prototypes.map((prototype, index) => {
      const fieldRowsOverride = overrides.fieldRows?.[prototype.prototype];
      return {
        prototype: prototype.prototype,
        schema: prototype.schema,
        baseline: snapshot(values, prototype, 1_000 + index, fieldRowsOverride),
        after_churn: snapshot(values, prototype, 1_100 + index, fieldRowsOverride),
        after_vacuum: snapshot(values, prototype, 1_110 + index, fieldRowsOverride),
        vacuum_duration_ms: 25 + index,
      };
    }),
  };

  const resourceFiles = ['runner-resources-before.txt', 'runner-resources-after.txt'];
  for (const filename of resourceFiles) writeFileSync(path.join(directory, filename), 'fixture\n');
  const provenance = {
    packet_contract_version: 2,
    generated_at: generatedAt,
    repository: 'RusTokRs/RusTok',
    commit: '0123456789abcdef0123456789abcdef01234567',
    ref: 'refs/heads/main',
    run_id: scale === '100k' ? '100' : '101',
    run_attempt: '1',
    job: 'index-storage-scale',
    postgres_image: 'postgres:16',
    runner_os: 'Linux',
    runner_arch: 'X64',
    scale,
    repetitions: 3,
    churn_cycles: 5,
    source_workload_names: readWorkloads,
    expected_product_rows: values.products,
    expected_entity_rows: values.entities,
    expected_eav_field_rows: values.fields,
    expected_link_rows: values.links,
    reports: ['read-report.json', 'mutation-report.json', 'maintenance-report.json'],
    runner_resource_files: resourceFiles,
    ...overrides.provenance,
  };

  writeJson(path.join(directory, 'read-report.json'), read);
  writeJson(path.join(directory, 'mutation-report.json'), mutation);
  writeJson(path.join(directory, 'maintenance-report.json'), maintenance);
  writeJson(path.join(directory, 'provenance.json'), provenance);
  return directory;
}

const runComparator = (inputs, output) => {
  const args = [scriptPath];
  for (const input of inputs) args.push('--input', input);
  args.push('--output', output);
  return spawnSync('node', args, { encoding: 'utf8' });
};
const withFixture = (callback) => {
  const root = mkdtempSync(path.join(tmpdir(), 'rustok-index-comparison-'));
  try {
    callback(root);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
};

const expectFailure = (root, packetOverrides, pattern) => {
  const packet = writePacket(root, '100k', packetOverrides);
  const result = runComparator([packet], path.join(root, 'comparison'));
  assert.notEqual(result.status, 0, 'expected comparator to fail closed');
  assert.match(result.stderr, pattern);
};

test('same-commit complete 100k and 1m evidence is decision-ready', () => {
  withFixture((root) => {
    const lower = writePacket(root, '100k');
    const upper = writePacket(root, '1m');
    const output = path.join(root, 'comparison');
    const result = runComparator([lower, upper], output);
    assert.equal(result.status, 0, result.stderr || result.stdout);
    const report = JSON.parse(readFileSync(path.join(output, 'comparison.json'), 'utf8'));
    assert.equal(report.decision_ready, true);
    assert.equal(report.decision_contract.same_database_settings, true);
    assert.equal(report.decision_contract.same_source_oracle_shape, true);
    assert.equal(report.cross_scale_ratios.source_workloads[0].result_rows_ratio_1m_to_100k, 10);
  });
});

test('one valid scale remains non-decision-ready', () => {
  withFixture((root) => {
    const packet = writePacket(root, '100k');
    const output = path.join(root, 'comparison');
    const result = runComparator([packet], output);
    assert.equal(result.status, 0, result.stderr || result.stdout);
    const report = JSON.parse(readFileSync(path.join(output, 'comparison.json'), 'utf8'));
    assert.equal(report.decision_ready, false);
  });
});

test('rejects missing read execution timing', () => {
  withFixture((root) => expectFailure(root, {
    readEvidence: { jsonb: { status_equality: { execution_time_ms: null } } },
  }, /execution_time_ms must be a non-negative number/));
});

test('rejects malformed EXPLAIN plan', () => {
  withFixture((root) => expectFailure(root, {
    readEvidence: { jsonb: { status_equality: { plan: [] } } },
  }, /must contain one EXPLAIN JSON plan/));
});

test('rejects missing mutation WAL metric', () => {
  withFixture((root) => expectFailure(root, {
    mutationEvidence: { jsonb: { update_product_batch: { maximum_node_wal_bytes: null } } },
  }, /maximum_node_wal_bytes must be a non-negative integer/));
});

test('rejects candidate result drift from source oracle', () => {
  withFixture((root) => expectFailure(root, {
    candidateDigest: { typed_eav: { status_equality: 'ffffffffffffffffffffffffffffffff' } },
  }, /typed_eav\/status_equality differs from source oracle/));
});

test('rejects maintenance EAV field cardinality drift', () => {
  withFixture((root) => expectFailure(root, {
    fieldRows: { typed_eav: scaleValues['100k'].fields - 1 },
  }, /typed_eav\/baseline maintenance cardinality mismatch/));
});

test('rejects report repetition drift', () => {
  withFixture((root) => expectFailure(root, { readRepetitions: 2 }, /must contain 3 read repetitions/));
});

test('rejects cross-scale commit mismatch', () => {
  withFixture((root) => {
    const lower = writePacket(root, '100k');
    const upper = writePacket(root, '1m', {
      provenance: { commit: 'ffffffffffffffffffffffffffffffffffffffff' },
    });
    const result = runComparator([lower, upper], path.join(root, 'comparison'));
    assert.notEqual(result.status, 0, 'expected comparator to fail closed');
    assert.match(result.stderr, /cross-scale commit mismatch/);
  });
});

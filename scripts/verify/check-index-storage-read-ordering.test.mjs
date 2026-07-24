#!/usr/bin/env node

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, mkdirSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';

const script = path.resolve('scripts/verify/check-index-storage-read-ordering.mjs');
const prototypes = [
  { prototype: 'jsonb', relation: 'idx_bench_jsonb.entity' },
  { prototype: 'typed_eav', relation: 'idx_bench_eav.entity' },
  { prototype: 'hot_projection', relation: 'idx_bench_hot.product' },
];
const workloads = [
  'status_equality',
  'price_range_sort',
  'multi_value_tag',
  'two_hop_channel_filter',
  'keyset_page',
  'exact_count',
];

const sql = (relation, workload) => {
  if (workload === 'exact_count') {
    return `SELECT count(*)::bigint AS result_count FROM ${relation}`;
  }
  if (workload === 'price_range_sort' || workload === 'keyset_page') {
    return `SELECT entity_id, price_minor FROM ${relation} ORDER BY price_minor, entity_id LIMIT 100`;
  }
  return `SELECT entity_id FROM ${relation} ORDER BY entity_id LIMIT 100`;
};

const report = () => ({
  source_workloads: workloads.map((name) => ({
    name,
    sql: `${sql('idx_bench_source.product', name)}   \n`,
  })),
  prototypes: prototypes.map(({ prototype, relation }) => ({
    prototype,
    workloads: workloads.map((name) => ({
      name,
      sql: sql(relation, name),
    })),
  })),
});

const withPacket = (mutate, callback) => {
  const root = mkdtempSync(path.join(tmpdir(), 'rustok-index-read-ordering-'));
  try {
    const value = report();
    mutate?.(value);
    mkdirSync(root, { recursive: true });
    writeFileSync(path.join(root, 'read-report.json'), `${JSON.stringify(value, null, 2)}\n`, 'utf8');
    callback(root);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
};

const run = (root) => spawnSync(process.execPath, [script, '--input', root], { encoding: 'utf8' });

const expectFailure = (mutate, pattern) => {
  withPacket(mutate, (root) => {
    const result = run(root);
    assert.notEqual(result.status, 0, 'expected terminal ordering preflight to fail');
    assert.match(result.stderr, pattern);
  });
};

test('accepts canonical terminal ordering with trailing whitespace', () => {
  withPacket(null, (root) => {
    const result = run(root);
    assert.equal(result.status, 0, result.stderr || result.stdout);
  });
});

test('accepts comment tokens inside strings and comments after executable ordering', () => {
  withPacket((value) => {
    value.source_workloads[0].sql = [
      "SELECT entity_id, '-- not a comment /* still text */' AS note",
      'FROM idx_bench_source.product',
      'ORDER BY entity_id LIMIT 100',
      '/* archived explanation after the executable query */',
    ].join('\n');
  }, (root) => {
    const result = run(root);
    assert.equal(result.status, 0, result.stderr || result.stdout);
  });
});

test('rejects a source ordering marker that exists only in a nested query', () => {
  expectFailure((value) => {
    value.source_workloads[0].sql = [
      'SELECT entity_id',
      'FROM (',
      '  SELECT entity_id FROM idx_bench_source.product ORDER BY entity_id LIMIT 100',
      ') nested_source',
      'LIMIT 100',
    ].join('\n');
  }, /source\/status_equality\.sql must end with canonical ordering marker/u);
});

test('rejects a candidate ordering marker that exists only in a block comment', () => {
  expectFailure((value) => {
    value.prototypes[0].workloads[4].sql = [
      'SELECT entity_id, price_minor FROM idx_bench_jsonb.entity',
      'LIMIT 100',
      '/* ORDER BY price_minor, entity_id LIMIT 100 */',
    ].join('\n');
  }, /jsonb\/keyset_page\.sql must end with canonical ordering marker/u);
});

test('rejects a terminal ordering marker hidden in a line comment', () => {
  expectFailure((value) => {
    value.source_workloads[0].sql = [
      'SELECT entity_id FROM idx_bench_source.product LIMIT 100',
      '-- ORDER BY entity_id LIMIT 100',
    ].join('\n');
  }, /source\/status_equality\.sql must end with canonical ordering marker/u);
});

test('rejects an ordering marker hidden in a dollar-quoted string', () => {
  expectFailure((value) => {
    value.prototypes[1].workloads[0].sql = [
      'SELECT entity_id, $ordering$ORDER BY entity_id LIMIT 100$ordering$ AS note',
      'FROM idx_bench_eav.entity LIMIT 100',
    ].join('\n');
  }, /typed_eav\/status_equality\.sql must end with canonical ordering marker/u);
});

test('rejects an ordering marker hidden after an escaped quote in an E string', () => {
  expectFailure((value) => {
    value.source_workloads[0].sql = "SELECT E'payload\\' ORDER BY entity_id LIMIT 100' --'";
  }, /source\/status_equality\.sql must end with canonical ordering marker/u);
});

test('rejects unterminated SQL comments before ordering validation', () => {
  expectFailure((value) => {
    value.prototypes[2].workloads[0].sql = [
      'SELECT entity_id FROM idx_bench_hot.product',
      '/* ORDER BY entity_id LIMIT 100',
    ].join('\n');
  }, /contains an unterminated block comment/u);
});

test('rejects workload order drift before checking SQL text', () => {
  expectFailure((value) => {
    [value.source_workloads[0], value.source_workloads[1]] = [
      value.source_workloads[1],
      value.source_workloads[0],
    ];
  }, /source workload order mismatch/u);
});

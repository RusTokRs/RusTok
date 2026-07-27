#!/usr/bin/env node

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';

const validator = path.resolve('scripts/verify/validate-index-storage-evidence.mjs');
const comparator = path.resolve('scripts/verify/compare-index-storage-evidence.mjs');
const prototypes = ['jsonb'];
const workloads = [
  'status_equality',
  'price_range_sort',
  'multi_value_tag',
  'two_hop_channel_filter',
  'keyset_page',
  'exact_count',
];
const databaseMetadata = () => ({
  version: 'PostgreSQL 16 fixture',
  server_version_num: '160000',
  shared_buffers: '128MB',
  effective_cache_size: '4GB',
  work_mem: '4MB',
  random_page_cost: '4',
  jit: 'off',
  standard_conforming_strings: 'on',
  timezone: 'UTC',
  date_style: 'ISO, YMD',
  extra_float_digits: '3',
});

const readSql = (relation, workload) => {
  if (workload === 'exact_count') return `SELECT count(*)::bigint AS result_count FROM ${relation}`;
  if (workload === 'price_range_sort' || workload === 'keyset_page') {
    return `SELECT entity_id, price_minor FROM ${relation} ORDER BY price_minor, entity_id LIMIT 100`;
  }
  return `SELECT entity_id FROM ${relation} ORDER BY entity_id LIMIT 100`;
};

const report = () => ({
  database: databaseMetadata(),
  source_workloads: workloads.map((name) => ({
    name,
    sql: readSql('idx_bench_source.product', name),
  })),
  prototypes: prototypes.map((prototype) => ({
    prototype,
    workloads: workloads.map((name) => ({
      name,
      sql: readSql(`idx_bench_${prototype}.entity`, name),
    })),
  })),
});

const withPacket = (mutate, callback) => {
  const root = mkdtempSync(path.join(tmpdir(), 'rustok-index-standalone-'));
  try {
    const value = report();
    mutate?.(value);
    writeFileSync(path.join(root, 'read-report.json'), `${JSON.stringify(value, null, 2)}\n`, 'utf8');
    for (const filename of ['mutation-report.json', 'maintenance-report.json']) {
      writeFileSync(
        path.join(root, filename),
        `${JSON.stringify({ database: databaseMetadata() }, null, 2)}\n`,
        'utf8',
      );
    }
    callback(root);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
};

const runValidator = (root) => spawnSync(process.execPath, [validator], {
  encoding: 'utf8',
  env: {
    ...process.env,
    INDEX_BENCH_SCALE: '100k',
    INDEX_BENCH_EVIDENCE_ROOT: root,
  },
});
const runComparator = (...args) => spawnSync(process.execPath, [comparator, ...args], {
  encoding: 'utf8',
});

const missingMutation = /missing evidence file: .*mutation-report\.json/u;

test('direct validator rejects non-executable terminal ordering before its core', () => {
  withPacket((value) => {
    value.source_workloads[0].sql = [
      'SELECT entity_id FROM idx_bench_source.product LIMIT 100',
      '-- ORDER BY entity_id LIMIT 100',
    ].join('\n');
  }, (root) => {
    const result = runValidator(root);
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /\[validate-index-storage-evidence\]/u);
    assert.match(result.stderr, /must end with canonical ordering marker/u);
    assert.doesNotMatch(result.stderr, missingMutation);
  });
});

test('direct comparator rejects nested-only terminal ordering before its core', () => {
  withPacket((value) => {
    value.prototypes[0].workloads[4].sql = [
      'SELECT entity_id, price_minor FROM (',
      '  SELECT entity_id, price_minor FROM idx_bench_jsonb.entity',
      '  ORDER BY price_minor, entity_id LIMIT 100',
      ') nested_page LIMIT 100',
    ].join('\n');
  }, (root) => {
    const result = runComparator('--input', root, '--output', path.join(root, 'comparison'));
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /\[compare-index-storage-evidence\]/u);
    assert.match(result.stderr, /must end with canonical ordering marker/u);
    assert.doesNotMatch(result.stderr, missingMutation);
  });
});

test('direct validator reaches the byte-preserved core after a valid preflight', () => {
  withPacket(null, (root) => {
    const result = runValidator(root);
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /read\.generated_at must be a non-empty string/u);
    assert.doesNotMatch(result.stderr, /must end with canonical ordering marker/u);
  });
});

test('direct comparator forwards help to the byte-preserved core', () => {
  const result = runComparator('--help');
  assert.equal(result.status, 0, result.stderr || result.stdout);
  assert.match(result.stdout, /compare-index-storage-evidence\.mjs --input <dir>/u);
});

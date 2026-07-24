#!/usr/bin/env node

import { test } from 'node:test';
import assert from 'node:assert/strict';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';

const script = path.resolve('scripts/verify/index-storage-tooling.mjs');
const readWorkloads = [
  'status_equality',
  'price_range_sort',
  'multi_value_tag',
  'two_hop_channel_filter',
  'keyset_page',
  'exact_count',
];
const prototypes = [
  { prototype: 'jsonb', relation: 'idx_bench_jsonb.entity' },
  { prototype: 'typed_eav', relation: 'idx_bench_eav.entity' },
  { prototype: 'hot_projection', relation: 'idx_bench_hot.product' },
];

const run = (...args) => spawnSync(process.execPath, [script, ...args], {
  encoding: 'utf8',
});

const readSql = (relation, workload) => {
  if (workload === 'exact_count') {
    return `SELECT count(*)::bigint AS result_count FROM ${relation}`;
  }
  if (workload === 'price_range_sort' || workload === 'keyset_page') {
    return `SELECT entity_id, price_minor FROM ${relation} ORDER BY price_minor, entity_id LIMIT 100`;
  }
  return `SELECT entity_id FROM ${relation} ORDER BY entity_id LIMIT 100`;
};

const orderingReport = () => ({
  source_workloads: readWorkloads.map((name) => ({
    name,
    sql: readSql('idx_bench_source.product', name),
  })),
  prototypes: prototypes.map(({ prototype, relation }) => ({
    prototype,
    workloads: readWorkloads.map((name) => ({ name, sql: readSql(relation, name) })),
  })),
});

const withOrderingPacket = (mutate, callback) => {
  const root = mkdtempSync(path.join(tmpdir(), 'rustok-index-tooling-ordering-'));
  try {
    const report = orderingReport();
    mutate(report);
    writeFileSync(path.join(root, 'read-report.json'), `${JSON.stringify(report, null, 2)}\n`, 'utf8');
    callback(root);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
};

test('prints the stable Index storage tooling command surface', () => {
  const result = run('--help');
  assert.equal(result.status, 0, result.stderr);
  for (const command of ['contract', 'fixtures', 'packet', 'compare', 'hash', 'prepare', 'render', 'verify-adr']) {
    assert.match(result.stdout, new RegExp(`\\b${command}\\b`, 'u'));
  }
});

test('forwards hash help to the exact-byte comparison helper', () => {
  const result = run('hash', '--help');
  assert.equal(result.status, 0, result.stderr);
  assert.match(result.stdout, /hash-index-storage-comparison\.mjs <comparison\.json>/u);
});

test('forwards comparator help without rewriting its arguments', () => {
  const result = run('compare', '--help');
  assert.equal(result.status, 0, result.stderr);
  assert.match(result.stdout, /compare-index-storage-evidence\.mjs --input <dir>/u);
});

test('forwards decision preparation help without rewriting its arguments', () => {
  const result = run('prepare', '--help');
  assert.equal(result.status, 0, result.stderr);
  assert.match(result.stdout, /prepare-index-storage-decision\.mjs/u);
  assert.match(result.stdout, /--selected <jsonb\|typed_eav\|hot_projection>/u);
});

test('forwards ADR finalization help without rewriting its arguments', () => {
  const result = run('render', '--help');
  assert.equal(result.status, 0, result.stderr);
  assert.match(result.stdout, /finalize-index-storage-adr\.mjs/u);
  assert.match(result.stdout, /--comparison <comparison\.json>/u);
});

test('forwards ADR verification help without rewriting its arguments', () => {
  const result = run('verify-adr', '--help');
  assert.equal(result.status, 0, result.stderr);
  assert.match(result.stdout, /verify-index-storage-adr\.mjs/u);
  assert.match(result.stdout, /--adr <adr\.md>/u);
});

test('rejects unsupported packet scales before invoking the validator', () => {
  const result = run('packet', '--scale', '10m');
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /packet --scale must be smoke, 100k, or 1m/u);
});

test('packet runs terminal ordering preflight before the canonical validator', () => {
  withOrderingPacket((report) => {
    report.source_workloads[0].sql = [
      'SELECT entity_id FROM (',
      '  SELECT entity_id FROM idx_bench_source.product ORDER BY entity_id LIMIT 100',
      ') nested_source LIMIT 100',
    ].join('\n');
  }, (root) => {
    const result = run('packet', '--scale', '100k', '--root', root);
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /check-index-storage-read-ordering/u);
    assert.match(result.stderr, /must end with canonical ordering marker/u);
    assert.doesNotMatch(result.stderr, /missing evidence file: .*mutation-report\.json/u);
  });
});

test('compare runs terminal ordering preflight before the canonical comparator', () => {
  withOrderingPacket((report) => {
    report.prototypes[0].workloads[4].sql = [
      'SELECT entity_id, price_minor FROM idx_bench_jsonb.entity LIMIT 100',
      '/* ORDER BY price_minor, entity_id LIMIT 100 */',
    ].join('\n');
  }, (root) => {
    const result = run('compare', '--input', root, '--output', path.join(root, 'comparison'));
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /check-index-storage-read-ordering/u);
    assert.match(result.stderr, /must end with canonical ordering marker/u);
    assert.doesNotMatch(result.stderr, /missing evidence file: .*mutation-report\.json/u);
  });
});

test('rejects arguments for aggregate commands', () => {
  const contract = run('contract', '--unexpected');
  assert.notEqual(contract.status, 0);
  assert.match(contract.stderr, /contract does not accept arguments/u);

  const fixtures = run('fixtures', '--unexpected');
  assert.notEqual(fixtures.status, 0);
  assert.match(fixtures.stderr, /fixtures does not accept arguments/u);
});

test('rejects unknown commands', () => {
  const result = run('publish');
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /unknown command: publish/u);
});

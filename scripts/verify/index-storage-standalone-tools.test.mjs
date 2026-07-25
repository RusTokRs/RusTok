#!/usr/bin/env node

import { test } from 'node:test';
import assert from 'node:assert/strict';
import {
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  readdirSync,
  rmSync,
  writeFileSync,
} from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';

import { runComparatorCoreWithAtomicComparison } from './compare-index-storage-evidence.mjs';

const validator = path.resolve('scripts/verify/validate-index-storage-evidence.mjs');
const comparator = path.resolve('scripts/verify/compare-index-storage-evidence.mjs');
const prototypes = ['jsonb', 'typed_eav', 'hot_projection'];
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
const quietStream = { write: () => true };
const stagingEntries = (output) => readdirSync(output)
  .filter((entry) => entry.startsWith('.comparison-staging-'));
const stagedOutputFrom = (args) => {
  const outputIndex = args.lastIndexOf('--output');
  assert.notEqual(outputIndex, -1);
  return args[outputIndex + 1];
};

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

test('direct comparator revokes stale comparison before ordering preflight', () => {
  withPacket((value) => {
    value.prototypes[0].workloads[4].sql = [
      'SELECT entity_id, price_minor FROM (',
      '  SELECT entity_id, price_minor FROM idx_bench_jsonb.entity',
      '  ORDER BY price_minor, entity_id LIMIT 100',
      ') nested_page LIMIT 100',
    ].join('\n');
  }, (root) => {
    const output = path.join(root, 'comparison');
    mkdirSync(output);
    writeFileSync(path.join(output, 'comparison.json'), '{"stale":true}\n', 'utf8');

    const result = runComparator('--input', root, '--output', output);
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /\[compare-index-storage-evidence\]/u);
    assert.match(result.stderr, /must end with canonical ordering marker/u);
    assert.doesNotMatch(result.stderr, missingMutation);
    assert.equal(existsSync(path.join(output, 'comparison.json')), false);
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

test('direct comparator rejects help combined with other arguments', () => {
  const result = runComparator('--help', '--output', 'comparison');
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /help must be the only argument/u);
  assert.equal(result.stdout, '');
});

test('direct comparator rejects duplicate output before its core', () => {
  const result = runComparator(
    '--input', 'missing-evidence',
    '--output', 'first-comparison',
    '--output', 'second-comparison',
  );
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /--output was provided more than once/u);
  assert.doesNotMatch(result.stderr, /missing evidence file/u);
});

test('comparator publishes finalized JSON last and removes staging', () => {
  const root = mkdtempSync(path.join(tmpdir(), 'rustok-index-comparator-publish-'));
  const output = path.join(root, 'comparison');
  try {
    mkdirSync(output);
    writeFileSync(path.join(output, 'comparison.json'), '{"stale":true}\n', 'utf8');
    writeFileSync(path.join(output, 'comparison.md'), 'stale markdown\n', 'utf8');

    const spawn = (_executable, args) => {
      const staged = stagedOutputFrom(args);
      writeFileSync(path.join(staged, 'comparison.json'), '{"core":true}\n', 'utf8');
      writeFileSync(path.join(staged, 'comparison.md'), 'core markdown\n', 'utf8');
      return { status: 0, stdout: '', stderr: '' };
    };
    const finalizeComparison = ({ output: staged }) => {
      writeFileSync(path.join(staged, 'comparison.json'), '{"finalized":true}\n', 'utf8');
      writeFileSync(path.join(staged, 'comparison.md'), 'finalized markdown\n', 'utf8');
    };

    const status = runComparatorCoreWithAtomicComparison({
      inputs: ['packet-100k', 'packet-1m'],
      output,
      spawn,
      finalizeComparison,
      stdout: quietStream,
      stderr: quietStream,
    });

    assert.equal(status, 0);
    assert.equal(readFileSync(path.join(output, 'comparison.json'), 'utf8'), '{"finalized":true}\n');
    assert.equal(readFileSync(path.join(output, 'comparison.md'), 'utf8'), 'finalized markdown\n');
    assert.deepEqual(stagingEntries(output), []);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test('comparator core failure cannot publish a partial decision input', () => {
  const root = mkdtempSync(path.join(tmpdir(), 'rustok-index-comparator-core-failure-'));
  const output = path.join(root, 'comparison');
  try {
    mkdirSync(output);
    writeFileSync(path.join(output, 'comparison.json'), '{"stale":true}\n', 'utf8');
    const spawn = (_executable, args) => {
      const staged = stagedOutputFrom(args);
      writeFileSync(path.join(staged, 'comparison.json'), '{"partial":', 'utf8');
      return { status: 1, stdout: '', stderr: 'core failed\n' };
    };

    const status = runComparatorCoreWithAtomicComparison({
      inputs: ['packet-100k', 'packet-1m'],
      output,
      spawn,
      stdout: quietStream,
      stderr: quietStream,
    });

    assert.equal(status, 1);
    assert.equal(existsSync(path.join(output, 'comparison.json')), false);
    assert.deepEqual(stagingEntries(output), []);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test('comparison post-processing failure leaves no decision input', () => {
  const root = mkdtempSync(path.join(tmpdir(), 'rustok-index-comparator-finalize-failure-'));
  const output = path.join(root, 'comparison');
  try {
    mkdirSync(output);
    writeFileSync(path.join(output, 'comparison.json'), '{"stale":true}\n', 'utf8');
    const spawn = (_executable, args) => {
      const staged = stagedOutputFrom(args);
      writeFileSync(path.join(staged, 'comparison.json'), '{"core":true}\n', 'utf8');
      writeFileSync(path.join(staged, 'comparison.md'), 'core markdown\n', 'utf8');
      return { status: 0, stdout: '', stderr: '' };
    };

    assert.throws(
      () => runComparatorCoreWithAtomicComparison({
        inputs: ['packet-100k', 'packet-1m'],
        output,
        spawn,
        finalizeComparison: () => {
          throw new Error('methodology finalization failed');
        },
        stdout: quietStream,
        stderr: quietStream,
      }),
      /methodology finalization failed/u,
    );
    assert.equal(existsSync(path.join(output, 'comparison.json')), false);
    assert.deepEqual(stagingEntries(output), []);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

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
];
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
const runProvenance = () => ({
  repository: process.env.GITHUB_REPOSITORY ?? 'RusTokRs/RusTok',
  commit: process.env.GITHUB_SHA ?? 'a'.repeat(40),
  ref: process.env.GITHUB_REF ?? 'refs/heads/agent/index-evidence',
  run_id: process.env.GITHUB_RUN_ID ?? '123456',
  run_attempt: process.env.GITHUB_RUN_ATTEMPT ?? '1',
  job: process.env.GITHUB_JOB ?? 'evidence',
  runner_os: process.env.RUNNER_OS ?? 'Linux',
  runner_arch: process.env.RUNNER_ARCH ?? 'X64',
});

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
  generated_at: '2026-07-25T00:00:00Z',
  database: databaseMetadata(),
  provenance: runProvenance(),
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
    const read = report();
    const mutation = {
      generated_at: '2026-07-25T00:01:00Z',
      database: databaseMetadata(),
      provenance: runProvenance(),
    };
    const maintenance = {
      generated_at: '2026-07-25T00:02:00Z',
      database: databaseMetadata(),
      provenance: runProvenance(),
    };
    mutate?.(read, mutation, maintenance);
    mkdirSync(root, { recursive: true });
    writeFileSync(path.join(root, 'read-report.json'), `${JSON.stringify(read, null, 2)}\n`, 'utf8');
    writeFileSync(path.join(root, 'mutation-report.json'), `${JSON.stringify(mutation, null, 2)}\n`, 'utf8');
    writeFileSync(path.join(root, 'maintenance-report.json'), `${JSON.stringify(maintenance, null, 2)}\n`, 'utf8');
    callback(root);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
};

const run = (root, env = process.env) => spawnSync(
  process.execPath,
  [script, '--input', root],
  { encoding: 'utf8', env },
);

const expectFailure = (mutate, pattern) => {
  withPacket(mutate, (root) => {
    const result = run(root);
    assert.notEqual(result.status, 0, 'expected evidence preflight to fail');
    assert.match(result.stderr, pattern);
  });
};

test('accepts canonical terminal ordering with trailing whitespace', () => {
  withPacket(null, (root) => {
    const result = run(root);
    assert.equal(result.status, 0, result.stderr || result.stdout);
  });
});

test('rejects missing deterministic session metadata', () => {
  expectFailure((value) => {
    delete value.database.standard_conforming_strings;
  }, /read\.database fields mismatch/u);
});

test('rejects deterministic session metadata drift', () => {
  expectFailure((value) => {
    value.database.timezone = 'Europe/Moscow';
  }, /read\.database\.timezone must be UTC; got Europe\/Moscow/u);
});

test('rejects mutation database metadata drift from the read session', () => {
  expectFailure((_read, mutation) => {
    mutation.database.work_mem = '8MB';
  }, /mutation-report\.json\.database\.work_mem must match read-report\.json database metadata/u);
});

test('rejects maintenance report without exact database metadata fields', () => {
  expectFailure((_read, _mutation, maintenance) => {
    delete maintenance.database.version;
  }, /maintenance-report\.json\.database fields mismatch/u);
});

test('rejects a mutation report without observed database metadata', () => {
  expectFailure((_read, mutation) => {
    delete mutation.database;
  }, /mutation-report\.json\.database must be an object/u);
});

test('rejects missing report provenance fields', () => {
  expectFailure((_read, mutation) => {
    delete mutation.provenance.run_attempt;
  }, /mutation-report\.json\.provenance fields mismatch/u);
});

test('rejects cross-report run provenance drift', () => {
  expectFailure((_read, mutation) => {
    mutation.provenance.commit = 'b'.repeat(40);
  }, /mutation-report\.json\.provenance\.commit must match read-report\.json run provenance/u);
});

test('rejects report provenance from a different current GitHub job', () => {
  withPacket(null, (root) => {
    const differentCommit = runProvenance().commit === 'b'.repeat(40)
      ? 'c'.repeat(40)
      : 'b'.repeat(40);
    const result = run(root, {
      ...process.env,
      INDEX_BENCH_REQUIRE_GITHUB_PROVENANCE: '1',
      GITHUB_REPOSITORY: runProvenance().repository,
      GITHUB_SHA: differentCommit,
      GITHUB_REF: runProvenance().ref,
      GITHUB_RUN_ID: runProvenance().run_id,
      GITHUB_RUN_ATTEMPT: runProvenance().run_attempt,
      GITHUB_JOB: runProvenance().job,
      RUNNER_OS: runProvenance().runner_os,
      RUNNER_ARCH: runProvenance().runner_arch,
    });
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /read\.provenance\.commit must match current GITHUB_SHA/u);
  });
});

test('rejects stale packet provenance from another report run', () => {
  withPacket(null, (root) => {
    const staleCommit = runProvenance().commit === 'b'.repeat(40)
      ? 'c'.repeat(40)
      : 'b'.repeat(40);
    writeFileSync(path.join(root, 'provenance.json'), `${JSON.stringify({
      ...runProvenance(),
      commit: staleCommit,
      packet_contract_version: 2,
    }, null, 2)}\n`, 'utf8');
    const result = run(root);
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /provenance\.json\.provenance\.commit must match read-report\.json run provenance/u);
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
    value.prototypes[0].workloads[0].sql = [
      'SELECT entity_id, $ordering$ORDER BY entity_id LIMIT 100$ordering$ AS note',
      'FROM idx_bench_jsonb.entity LIMIT 100',
    ].join('\n');
  }, /jsonb\/status_equality\.sql must end with canonical ordering marker/u);
});

test('rejects an ordering marker hidden after an escaped quote in an E string', () => {
  expectFailure((value) => {
    value.source_workloads[0].sql = "SELECT E'payload\\' ORDER BY entity_id LIMIT 100' --'";
  }, /source\/status_equality\.sql must end with canonical ordering marker/u);
});

test('rejects unterminated SQL comments before ordering validation', () => {
  expectFailure((value) => {
    value.prototypes[0].workloads[0].sql = [
      'SELECT entity_id FROM idx_bench_jsonb.entity',
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

test('rejects help combined with evidence arguments', () => {
  const result = spawnSync(
    process.execPath,
    [script, '--help', '--input', 'missing-evidence'],
    { encoding: 'utf8' },
  );
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /help must be the only argument/u);
  assert.equal(result.stdout, '');
});

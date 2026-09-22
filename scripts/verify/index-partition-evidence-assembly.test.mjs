import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import {
  linkSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import test from 'node:test';

import {
  CAPTURE_CONTRACT,
  assemblePartitionPacket,
  readCaptureArtifacts,
} from './index-partition-evidence-assembly-core.mjs';
import {
  PLAN_DIGEST_CONTRACT,
  prepareManifest,
  sha256Hex,
} from './index-partition-evidence-core.mjs';

const digest = (character) => character.repeat(64);

const manifestInput = () => ({
  contract: 'index_partition_evidence_manifest_v1',
  repository: 'RusTokRs/RusTok',
  commit: '1'.repeat(40),
  run_key: 'fixture-run-1',
  postgres_image: 'postgres:16',
  strategy: 'tenant_hash',
  plan_digest_contract: PLAN_DIGEST_CONTRACT,
  modulus: 4,
  locales: ['en-US', 'ru-RU'],
  repetitions: { query: 2, mutation: 2, maintenance: 2, cutover: 1 },
  thresholds: {
    minimum_total_rows: 100,
    minimum_total_bytes: 1_000,
    minimum_distinct_tenants: 4,
    maximum_query_p95_regression_bps: 500,
    maximum_mutation_p95_regression_bps: 500,
    maximum_wal_amplification_bps: 11_000,
    maximum_partition_size_to_mean_bps: 13_000,
    maximum_cutover_lock_ms: 250,
  },
});

const rawValues = () => ({
  baseline: {
    generated_at: '2026-07-27T13:00:00Z',
    distinct_tenants: 8,
    tenant_predicate_audit: { total_templates: 12, tenant_scoped_templates: 12 },
    entities: { rows: 1_000, bytes: 80_000, digest: digest('a') },
    links: { rows: 2_000, bytes: 120_000, digest: digest('b') },
  },
  shadow: {
    generated_at: '2026-07-27T13:30:00Z',
    caught_up: true,
    foreign_keys_validated: true,
    orphan_links: 0,
    entities: {
      rows: 1_000,
      bytes: 82_000,
      digest: digest('a'),
      partition_bytes: [20_000, 20_500, 20_500, 21_000],
    },
    links: {
      rows: 2_000,
      bytes: 123_000,
      digest: digest('b'),
      partition_bytes: [30_000, 30_500, 31_000, 31_500],
    },
  },
  query: [
    {
      name: 'filter-sort-1',
      baseline_p95_ms: 10,
      shadow_p95_ms: 10.2,
      baseline_plan_digest: digest('c'),
      shadow_plan_digest: digest('c'),
    },
    {
      name: 'keyset-1',
      baseline_p95_ms: 5,
      shadow_p95_ms: 5.1,
      baseline_plan_digest: digest('d'),
      shadow_plan_digest: digest('d'),
    },
  ],
  mutation: [
    {
      name: 'upsert-1',
      baseline_p95_ms: 20,
      shadow_p95_ms: 20.4,
      baseline_wal_bytes: 1_000,
      shadow_wal_bytes: 1_040,
    },
    {
      name: 'delete-1',
      baseline_p95_ms: 10,
      shadow_p95_ms: 10.2,
      baseline_wal_bytes: 500,
      shadow_wal_bytes: 520,
    },
  ],
  maintenance: [
    {
      name: 'vacuum-1',
      baseline_vacuum_ms: 100,
      shadow_vacuum_ms: 104,
      baseline_dead_tuples: 20,
      shadow_dead_tuples: 18,
    },
    {
      name: 'vacuum-2',
      baseline_vacuum_ms: 101,
      shadow_vacuum_ms: 105,
      baseline_dead_tuples: 21,
      shadow_dead_tuples: 19,
    },
  ],
  cutover: [
    { lock_ms: 50, rollback_verified: true, production_relations_unchanged: true },
  ],
});

const createBundle = () => {
  const root = mkdtempSync(path.join(tmpdir(), 'rustok-index-partition-assembly-'));
  const manifest = prepareManifest(manifestInput());
  const manifestPath = path.join(root, 'manifest.json');
  const capturePath = path.join(root, 'capture.json');
  const raw = rawValues();
  const artifacts = {};
  for (const [role, value] of Object.entries(raw)) {
    const filename = `raw-${role}.json`;
    writeFileSync(path.join(root, filename), `${JSON.stringify(value, null, 2)}\n`, 'utf8');
    artifacts[role] = filename;
  }
  const capture = {
    contract: CAPTURE_CONTRACT,
    completed_at: '2026-07-27T14:00:00Z',
    run_provenance: {
      repository: manifest.repository,
      commit: manifest.commit,
      run_key: manifest.run_key,
      job: 'partition-evidence-fixture',
      runner_os: 'Linux',
      runner_arch: 'X64',
    },
    database: {
      version: 'PostgreSQL 16.4',
      server_version_num: '160004',
      jit: 'off',
      system_identifier: '1234567890123456789',
      database_name: 'rustok_index_partition_evidence_fixture',
    },
    artifacts,
  };
  writeFileSync(manifestPath, `${JSON.stringify(manifest, null, 2)}\n`, 'utf8');
  writeFileSync(capturePath, `${JSON.stringify(capture, null, 2)}\n`, 'utf8');
  return { root, manifest, manifestPath, capture, capturePath };
};

const withBundle = (callback) => {
  const bundle = createBundle();
  try {
    callback(bundle);
  } finally {
    rmSync(bundle.root, { recursive: true, force: true });
  }
};

test('assembly hashes exact raw bytes and emits a structurally validated packet', () => {
  withBundle(({ manifest, capture, capturePath, root }) => {
    const { packet } = assemblePartitionPacket({ manifest, capturePath, capture });
    assert.equal(packet.manifest.evidence_id, manifest.evidence_id);
    assert.equal(packet.baseline.entities.rows, 1_000);
    assert.equal(packet.query_runs.length, 2);
    for (const [role, filename] of Object.entries(capture.artifacts)) {
      assert.equal(packet.raw_artifacts[role], sha256Hex(readFileSync(path.join(root, filename))));
    }

    const queryPath = path.join(root, capture.artifacts.query);
    const before = packet.raw_artifacts.query;
    writeFileSync(queryPath, `${readFileSync(queryPath, 'utf8')}\n`, 'utf8');
    const { packet: after } = assemblePartitionPacket({ manifest, capturePath, capture });
    assert.notEqual(after.raw_artifacts.query, before, 'whitespace changes exact-byte identity');
    assert.deepEqual(after.query_runs, packet.query_runs, 'parsed measurements remain unchanged');
  });
});

test('capture artifact paths fail closed on traversal and duplicate file identity', () => {
  withBundle(({ root, capture, capturePath }) => {
    const traversal = structuredClone(capture);
    traversal.artifacts.baseline = '../outside.json';
    assert.throws(
      () => readCaptureArtifacts({ capturePath, capture: traversal }),
      /must stay inside the capture bundle/u,
    );

    const duplicate = structuredClone(capture);
    duplicate.artifacts.shadow = duplicate.artifacts.baseline;
    assert.throws(
      () => readCaptureArtifacts({ capturePath, capture: duplicate }),
      /artifact files must be unique/u,
    );

    const hardlinkName = 'raw-shadow-hardlink.json';
    linkSync(
      path.join(root, capture.artifacts.baseline),
      path.join(root, hardlinkName),
    );
    const hardlink = structuredClone(capture);
    hardlink.artifacts.shadow = hardlinkName;
    assert.throws(
      () => readCaptureArtifacts({ capturePath, capture: hardlink }),
      /artifact files must be unique/u,
    );
  });
});

test('assembler CLI publishes once and never aliases or overwrites retained inputs', () => {
  withBundle(({ root, manifestPath, capturePath, capture }) => {
    const script = path.resolve('scripts/verify/assemble-index-partition-evidence.mjs');
    const output = path.join(root, 'partition-packet.json');
    const run = (...args) => spawnSync(process.execPath, [script, ...args], { encoding: 'utf8' });

    const first = run('--manifest', manifestPath, '--capture', capturePath, '--output', output);
    assert.equal(first.status, 0, first.stderr);
    assert.match(first.stdout, /assembled [0-9a-f]{64}/u);

    const second = run('--manifest', manifestPath, '--capture', capturePath, '--output', output);
    assert.notEqual(second.status, 0);
    assert.match(second.stderr, /refusing to overwrite existing file/u);

    const aliased = run(
      '--manifest', manifestPath,
      '--capture', capturePath,
      '--output', path.join(root, capture.artifacts.baseline),
    );
    assert.notEqual(aliased.status, 0);
    assert.match(aliased.stderr, /must not alias a raw artifact path/u);

    const captureAlias = path.join(root, 'capture-hardlink.json');
    linkSync(manifestPath, captureAlias);
    const retainedAlias = run(
      '--manifest', manifestPath,
      '--capture', captureAlias,
      '--output', path.join(root, 'alias-packet.json'),
    );
    assert.notEqual(retainedAlias.status, 0);
    assert.match(retainedAlias.stderr, /must not alias the same file/u);
  });
});

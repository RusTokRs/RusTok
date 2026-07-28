import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';

import { assemblePartitionPacket } from './index-partition-evidence-assembly-core.mjs';
import {
  canonicalJson,
  prepareManifest,
  validatePartitionPacket,
} from './index-partition-evidence-core.mjs';

const reportScript = path.resolve('scripts/verify/render-index-partition-review.mjs');
const archiveManifestScript = path.resolve('scripts/verify/render-index-partition-archive-manifest.mjs');
const digest = (character) => character.repeat(64);

const writeJson = (filename, value) => writeFileSync(filename, `${JSON.stringify(value, null, 2)}\n`);

const manifestInput = () => ({
  contract: 'index_partition_evidence_manifest_v1',
  repository: 'RusTokRs/RusTok',
  commit: '1'.repeat(40),
  run_key: 'retained-review-fixture',
  postgres_image: 'postgres:16',
  strategy: 'tenant_hash',
  plan_digest_contract: 'normalized_partition_plan_v1',
  modulus: 4,
  locales: ['en-US', 'ru-RU'],
  repetitions: { query: 1, mutation: 1, maintenance: 1, cutover: 1 },
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

const buildBundle = () => {
  const root = mkdtempSync(path.join(os.tmpdir(), 'index-partition-review-'));
  const manifest = prepareManifest(manifestInput());
  const artifacts = {
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
    query: [{
      name: 'filter-sort-1',
      baseline_p95_ms: 10,
      shadow_p95_ms: 10.2,
      baseline_plan_digest: digest('c'),
      shadow_plan_digest: digest('c'),
    }],
    mutation: [{
      name: 'upsert-1',
      baseline_p95_ms: 20,
      shadow_p95_ms: 20.4,
      baseline_wal_bytes: 1_000,
      shadow_wal_bytes: 1_040,
    }],
    maintenance: [{
      name: 'vacuum-1',
      baseline_vacuum_ms: 100,
      shadow_vacuum_ms: 104,
      baseline_dead_tuples: 20,
      shadow_dead_tuples: 18,
    }],
    cutover: [{ lock_ms: 50, rollback_verified: true, production_relations_unchanged: true }],
  };
  for (const [role, value] of Object.entries(artifacts)) {
    writeJson(path.join(root, `${role}.json`), value);
  }
  const capture = {
    contract: 'index_partition_capture_v1',
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
      system_identifier: '7432859712345678901',
      database_name: 'rustok_index_partition_fixture',
    },
    artifacts: Object.fromEntries(Object.keys(artifacts).map((role) => [role, `${role}.json`])),
  };
  const capturePath = path.join(root, 'capture.json');
  writeJson(capturePath, capture);
  const { packet } = assemblePartitionPacket({ manifest, capturePath, capture });
  const admission = validatePartitionPacket(packet);
  writeJson(path.join(root, 'partition-packet.json'), packet);
  writeJson(path.join(root, 'admission.json'), admission);
  return { root };
};

const runScript = (script, root) => spawnSync(
  process.execPath,
  [script, '--root', root],
  { encoding: 'utf8' },
);

const runReport = (root) => runScript(reportScript, root);
const runArchiveManifest = (root) => runScript(archiveManifestScript, root);

const snapshot = (root) => Object.fromEntries(
  readdirSync(root).sort().map((filename) => [filename, readFileSync(path.join(root, filename))]),
);

test('renders a deterministic read-only nine-file retained bundle review', () => {
  const context = buildBundle();
  try {
    const before = snapshot(context.root);
    const first = runReport(context.root);
    const second = runReport(context.root);
    assert.equal(first.status, 0, first.stderr);
    assert.equal(second.status, 0, second.stderr);
    assert.equal(first.stderr, '');
    assert.equal(first.stdout, second.stdout);
    assert.match(first.stdout, /index_partition_retained_bundle_review_v1/u);
    assert.match(first.stdout, /Admission outcome: `admitted`/u);
    assert.match(first.stdout, /Retained file count: `9`/u);
    assert.match(first.stdout, /Recalculated admission matches saved admission: `true`/u);
    assert.match(first.stdout, /Production partition copy\/replay/u);
    assert.deepEqual(snapshot(context.root), before);
  } finally {
    rmSync(context.root, { recursive: true, force: true });
  }
});

test('prints a deterministic read-only admitted archive manifest', () => {
  const context = buildBundle();
  try {
    const before = snapshot(context.root);
    const first = runArchiveManifest(context.root);
    const second = runArchiveManifest(context.root);
    assert.equal(first.status, 0, first.stderr);
    assert.equal(second.status, 0, second.stderr);
    assert.equal(first.stderr, '');
    assert.equal(first.stdout, second.stdout);
    const manifest = JSON.parse(first.stdout);
    const { manifest_digest: manifestDigest, ...payload } = manifest;
    assert.equal(manifest.contract, 'index_partition_retained_archive_manifest_v1');
    assert.equal(manifest.digest_contract, 'canonical_json_without_manifest_digest_v1');
    assert.equal(manifest.source_review_contract, 'index_partition_retained_bundle_review_v1');
    assert.equal(manifest.admission_outcome, 'admitted');
    assert.equal(manifest.file_count, 9);
    assert.equal(manifest.files.length, 9);
    assert.equal(
      manifest.total_bytes,
      manifest.files.reduce((total, file) => total + file.bytes, 0),
    );
    assert.equal(
      manifestDigest,
      createHash('sha256').update(canonicalJson(payload)).digest('hex'),
    );
    assert.deepEqual(snapshot(context.root), before);
  } finally {
    rmSync(context.root, { recursive: true, force: true });
  }
});

test('refuses an archive manifest for a non-admitted retained bundle', () => {
  const context = buildBundle();
  try {
    const packetPath = path.join(context.root, 'partition-packet.json');
    const admissionPath = path.join(context.root, 'admission.json');
    const packet = JSON.parse(readFileSync(packetPath, 'utf8'));
    packet.manifest.thresholds.maximum_query_p95_regression_bps = 0;
    writeJson(packetPath, packet);
    writeJson(admissionPath, validatePartitionPacket(packet));
    const result = runArchiveManifest(context.root);
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /requires admission outcome admitted/u);
    assert.equal(result.stdout, '');
  } finally {
    rmSync(context.root, { recursive: true, force: true });
  }
});

test('rejects a saved admission that does not match recalculated packet admission', () => {
  const context = buildBundle();
  try {
    const filename = path.join(context.root, 'admission.json');
    const admission = JSON.parse(readFileSync(filename, 'utf8'));
    admission.outcome = 'keep_unpartitioned';
    writeJson(filename, admission);
    const result = runReport(context.root);
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /admission file does not match recalculated retained bundle content/u);
    assert.equal(result.stdout, '');
  } finally {
    rmSync(context.root, { recursive: true, force: true });
  }
});

test('rejects raw artifact drift from the retained packet', () => {
  const context = buildBundle();
  try {
    const filename = path.join(context.root, 'query.json');
    const query = JSON.parse(readFileSync(filename, 'utf8'));
    query[0].shadow_p95_ms = 12;
    writeJson(filename, query);
    const result = runReport(context.root);
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /packet file does not match recalculated retained bundle content/u);
    assert.equal(result.stdout, '');
  } finally {
    rmSync(context.root, { recursive: true, force: true });
  }
});

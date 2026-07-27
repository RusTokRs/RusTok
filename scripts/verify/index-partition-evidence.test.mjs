import assert from 'node:assert/strict';
import test from 'node:test';

import {
  PACKET_CONTRACT,
  computeEvidenceId,
  prepareManifest,
  renderShadowBootstrapSql,
  validatePartitionPacket,
} from './index-partition-evidence-core.mjs';

const digest = (character) => character.repeat(64);

const config = () => ({
  contract: 'index_partition_evidence_manifest_v1',
  repository: 'RusTokRs/RusTok',
  commit: '1'.repeat(40),
  run_key: 'fixture-run-1',
  postgres_image: 'postgres:16',
  strategy: 'tenant_hash',
  plan_digest_contract: 'normalized_partition_plan_v1',
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

const packet = () => {
  const manifest = prepareManifest(config());
  return {
    contract: PACKET_CONTRACT,
    completed_at: '2026-07-27T14:00:00Z',
    manifest,
    run_provenance: {
      repository: manifest.repository,
      commit: manifest.commit,
      run_key: manifest.run_key,
      job: 'partition-evidence-fixture',
      runner_os: 'Linux',
      runner_arch: 'X64',
    },
    raw_artifacts: {
      baseline: digest('1'),
      shadow: digest('2'),
      query: digest('3'),
      mutation: digest('4'),
      maintenance: digest('5'),
      cutover: digest('6'),
    },
    database: {
      version: 'PostgreSQL 16.4',
      server_version_num: '160004',
      jit: 'off',
      system_identifier: '7432859712345678901',
      database_name: 'rustok_index_partition_fixture',
    },
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
    query_runs: [
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
    mutation_runs: [
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
    maintenance_runs: [
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
    cutover_rehearsals: [
      { lock_ms: 50, rollback_verified: true, production_relations_unchanged: true },
    ],
  };
};

test('manifest identity and shadow bootstrap are deterministic and non-destructive', () => {
  const first = prepareManifest(config());
  const second = prepareManifest(config());
  assert.deepEqual(first, second);
  assert.equal(first.evidence_id, computeEvidenceId(config()));
  assert.equal(first.shadow_relations.entities.partitions.length, 4);
  assert.equal(first.shadow_relations.links.partitions.length, 4);

  const sql = renderShadowBootstrapSql(first);
  assert.match(sql, /PARTITION BY HASH \(tenant_id\)/u);
  assert.match(sql, /MODULUS 4, REMAINDER 3/u);
  assert.doesNotMatch(sql, /ALTER TABLE "index_entities"/u);
  assert.doesNotMatch(sql, /ALTER TABLE "index_links"/u);
  assert.doesNotMatch(sql, /DROP TABLE/u);
  assert.doesNotMatch(sql, /RENAME TO/u);
});

test('complete measured packet is admitted with calculated metrics', () => {
  const admission = validatePartitionPacket(packet());
  assert.equal(admission.outcome, 'admitted');
  assert.deepEqual(admission.reasons, []);
  assert.equal(admission.packet_digest.length, 64);
  assert.equal(admission.run_provenance.run_key, 'fixture-run-1');
  assert.equal(admission.measurements.tenant_predicate_coverage_bps, 10_000);
  assert.equal(admission.measurements.query_runs, 2);
  assert.equal(admission.measurements.mutation_runs, 2);
  assert.equal(admission.measurements.maintenance_runs, 2);
  assert.equal(admission.measurements.cutover_rehearsals, 1);
  assert.equal(admission.measurements.query_plan_regressions, 0);
  assert.equal(admission.measurements.entity_digest_matches, true);
  assert.equal(admission.measurements.link_digest_matches, true);
});

test('regressed packet stays unpartitioned with typed calculated reasons', () => {
  const value = packet();
  value.baseline.tenant_predicate_audit.tenant_scoped_templates = 11;
  value.shadow.links.digest = digest('e');
  value.shadow.orphan_links = 2;
  value.query_runs[0].shadow_plan_digest = digest('f');
  value.query_runs[0].shadow_p95_ms = 12;
  value.mutation_runs[0].shadow_wal_bytes = 1_500;
  value.shadow.entities.partition_bytes = [10_000, 10_000, 10_000, 52_000];
  value.cutover_rehearsals[0] = {
    lock_ms: 500,
    rollback_verified: false,
    production_relations_unchanged: false,
  };

  const admission = validatePartitionPacket(value);
  assert.equal(admission.outcome, 'keep_unpartitioned');
  const codes = new Set(admission.reasons.map((reason) => reason.code));
  for (const code of [
    'tenant_predicate_coverage',
    'link_digest_mismatch',
    'orphan_links',
    'query_plan_regressions',
    'query_latency_regression',
    'wal_amplification',
    'partition_size_skew',
    'cutover_lock_exceeded',
    'rollback_not_verified',
    'production_relations_changed',
  ]) {
    assert.equal(codes.has(code), true, `missing reason ${code}`);
  }
});

test('manifest tampering, provenance drift, and incomplete groups fail validation', () => {
  const tampered = packet();
  tampered.manifest.modulus = 8;
  assert.throws(
    () => validatePartitionPacket(tampered),
    /evidence_id does not match canonical manifest bytes/u,
  );

  const provenanceDrift = packet();
  provenanceDrift.run_provenance.commit = '2'.repeat(40);
  assert.throws(
    () => validatePartitionPacket(provenanceDrift),
    /run_provenance.commit must match the manifest/u,
  );

  const incomplete = packet();
  incomplete.query_runs.pop();
  assert.throws(
    () => validatePartitionPacket(incomplete),
    /packet.query_runs must contain exactly 2 runs/u,
  );
});

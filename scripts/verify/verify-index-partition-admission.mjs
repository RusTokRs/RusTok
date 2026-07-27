#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const read = (relative) => fs.readFileSync(path.join(root, relative), 'utf8');
const fail = (message) => {
  console.error(`[verify-index-partition-admission] ${message}`);
  process.exit(1);
};
const requireMarkers = (relative, markers) => {
  const source = read(relative);
  for (const marker of markers) {
    if (!source.includes(marker)) fail(`${relative} is missing ${marker}`);
  }
  return source;
};

const admissionPath = 'crates/rustok-index/src/infrastructure/postgres/partition_admission.rs';
const admission = requireMarkers(admissionPath, [
  'pub enum PartitionStrategy',
  'TenantHash { modulus: u16 }',
  'pub struct PartitionAdmissionPolicy',
  'pub struct PartitionBaselineEvidence',
  'pub struct PartitionMeasurementCoverage',
  'pub struct PartitionShadowEvidence',
  'pub struct PartitionEvidence',
  'pub enum PartitionAdmissionReason',
  'MissingQueryMeasurements',
  'MissingMutationMeasurements',
  'MissingMaintenanceMeasurements',
  'MissingCutoverRehearsal',
  'pub enum PartitionAdmissionOutcome',
  'KeepUnpartitioned',
  'Admitted(PartitionShadowPlan)',
  'pub struct PartitionShadowPlan',
  'pub fn evaluate_partition_admission',
  'minimum_total_rows',
  'minimum_total_bytes',
  'minimum_distinct_tenants',
  'required_tenant_predicate_coverage_bps != BASIS_POINTS',
  'partition admission requires 10000 basis points of tenant predicate coverage',
  'query_runs',
  'mutation_runs',
  'maintenance_runs',
  'cutover_rehearsals',
  'entity_digest_matches',
  'link_digest_matches',
  'shadow_caught_up',
  'foreign_keys_validated',
  'orphan_links',
  'query_plan_regressions',
  'query_p95_regression_bps',
  'mutation_p95_regression_bps',
  'wal_amplification_bps',
  'maximum_partition_size_to_mean_bps',
  'cutover_lock_ms',
  'evidence_id must be a lowercase 64-character SHA-256 hex digest',
  'partition modulus must be a power of two between 2 and 128',
  'rustok-index-partition',
  'tenant_hash_shadow_v1',
  'PARTITION BY HASH (tenant_id)',
  'FOR VALUES WITH (MODULUS',
  'index_entities_shadow_',
  'index_links_shadow_',
]);

for (const forbidden of [
  'ALTER TABLE "index_entities"',
  'ALTER TABLE "index_links"',
  'DROP TABLE "index_entities"',
  'DROP TABLE "index_links"',
  'RENAME TO index_entities',
  'RENAME TO index_links',
  'VACUUM FULL',
  'rustok_product',
  'rustok_content',
  'rustok_flex',
  'rustok_pricing',
  'rustok_inventory',
]) {
  if (admission.includes(forbidden)) fail(`${admissionPath} contains forbidden marker ${forbidden}`);
}

const admissionPosition = admission.indexOf('pub fn evaluate_partition_admission');
const planPosition = admission.indexOf('PartitionShadowPlan::new');
if (admissionPosition < 0 || planPosition < 0 || admissionPosition > planPosition) {
  fail('partition admission must run before a shadow plan is constructed');
}

requireMarkers('crates/rustok-index/src/infrastructure/postgres/partition_admission_tests.rs', [
  'admitted_plan_is_stable_and_shadow_only',
  'incomplete_or_regressed_evidence_keeps_storage_unpartitioned',
  'policy_strategy_and_evidence_validation_fail_closed',
  'PartitionMeasurementCoverage::new(3, 3, 3, 1)',
  'PartitionMeasurementCoverage::new(0, 0, 0, 0)',
  'PartitionAdmissionOutcome::KeepUnpartitioned',
  'PartitionAdmissionReason::MissingQueryMeasurements',
  'PartitionAdmissionReason::MissingMutationMeasurements',
  'PartitionAdmissionReason::MissingMaintenanceMeasurements',
  'PartitionAdmissionReason::MissingCutoverRehearsal',
  'PartitionAdmissionReason::EntityDigestMismatch',
  'PartitionAdmissionReason::ForeignKeysNotValidated',
  'PartitionAdmissionReason::QueryPlanRegressions',
  'PartitionAdmissionReason::WalAmplification',
  'PartitionAdmissionReason::CutoverLockExceeded',
  'PartitionAdmissionPolicy::new(1, 1, 2, 9_999',
  '!sql.contains("DROP TABLE")',
  '!sql.contains("RENAME TO")',
]);
requireMarkers('crates/rustok-index/src/infrastructure/postgres/mod.rs', [
  'mod partition_admission;',
  'mod partition_admission_tests;',
  'evaluate_partition_admission',
  'PartitionMeasurementCoverage',
  'PartitionShadowPlan',
]);
requireMarkers('crates/rustok-index/src/lib.rs', [
  'evaluate_partition_admission',
  'PartitionAdmissionOutcome',
  'PartitionMeasurementCoverage',
  'PartitionShadowPlan',
]);
requireMarkers('crates/rustok-index/docs/implementation-plan.md', [
  '- M3 partition admission and shadow planning: `complete`',
  '- [x] Add fail-closed partition admission and deterministic tenant-hash shadow',
  '- [ ] Execute retained PostgreSQL partition baseline/shadow evidence.',
  'tenant-predicate',
  'query/mutation',
  'They never rename, drop, or alter production',
]);
requireMarkers('DECISIONS/2026-07-24-index-storage-layout.md', [
  'The initial canonical tables remain unpartitioned because partitioning was not part of the M2 evidence',
  'a later measured shadow migration may introduce tenant-hash partitioning',
]);

console.log('[verify-index-partition-admission] OK');

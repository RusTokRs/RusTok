#!/usr/bin/env node

import { readFileSync } from 'node:fs';

const root = new URL('../../', import.meta.url);
const read = (path) => readFileSync(new URL(path, root), 'utf8');
const fail = (message) => {
  console.error(`[verify-index-storage-source-oracle] ${message}`);
  process.exit(1);
};

const sourceSql = read('ops/benches/src/index_storage/sql/source.rs');
const sqlModule = read('ops/benches/src/index_storage/sql/mod.rs');
const runner = read('ops/benches/src/index_storage/runner.rs');
const validator = read('scripts/verify/validate-index-storage-evidence.mjs');
const comparator = read('scripts/verify/compare-index-storage-evidence.mjs');
const comparatorFixture = read('scripts/verify/compare-index-storage-evidence.test.mjs');

const readWorkloads = [
  'status_equality',
  'price_range_sort',
  'multi_value_tag',
  'two_hop_channel_filter',
  'keyset_page',
  'exact_count',
];

for (const marker of [
  'pub fn workloads(context: &WorkloadContext) -> Vec<Workload>',
  'idx_bench_source.product',
  'idx_bench_source.variant',
  'idx_bench_source.variant_channel',
  'idx_bench_source.channel',
]) {
  if (!sourceSql.includes(marker)) fail(`source SQL oracle missing ${marker}`);
}
for (const workload of readWorkloads) {
  if (!sourceSql.includes(`name: "${workload}"`)) {
    fail(`source SQL oracle missing workload ${workload}`);
  }
}

for (const marker of [
  'pub fn source_workloads(config: &DatasetConfig) -> Vec<Workload>',
  'source::workloads(&WorkloadContext::new(config))',
]) {
  if (!sqlModule.includes(marker)) fail(`SQL module missing source oracle export ${marker}`);
}

for (const marker of [
  'pub source_workloads: Vec<SourceWorkloadReport>',
  'run_source_workloads(&db, &config.dataset)',
  'validate_semantic_parity(&source_workload_reports, &prototypes)',
  'differs from source oracle',
]) {
  if (!runner.includes(marker)) fail(`read runner missing source oracle contract ${marker}`);
}

for (const marker of [
  'read.source_workloads',
  "'source workload order'",
  "sourceWorkload.sql.includes('idx_bench_source.')",
  'differs from source oracle',
  'source_workload_names: canonicalReadWorkloads',
]) {
  if (!validator.includes(marker)) fail(`packet validator missing source oracle guard ${marker}`);
}
if (validator.includes('baselineReadWorkloads')) {
  fail('packet validator must not restore first-candidate read parity');
}

for (const marker of [
  'validateReadEvidence',
  'validateMutationEvidence',
  'requirePlan',
  'validateDatabase',
  'validateDataset',
  'validateProvenance',
  'validateSourceOracle',
  'validateReadReport',
  'validateMutationReport',
  'validateMaintenanceReport',
  'same_dataset_shape',
  'same_source_oracle_shape',
  'result_rows_ratio_1m_to_100k',
  'fail closed on report shape, metrics, plans, effects, and cardinalities',
  '### Source oracle',
]) {
  if (!comparator.includes(marker)) fail(`evidence comparator missing contract guard ${marker}`);
}
for (const legacy of [
  'values.filter(Number.isFinite)',
  'const numbers = (values)',
  'baselineReadWorkloads',
]) {
  if (comparator.includes(legacy)) fail(`evidence comparator restored lossy validation: ${legacy}`);
}

for (const marker of [
  "test('rejects missing read execution timing'",
  "test('rejects malformed EXPLAIN plan'",
  "test('rejects missing mutation WAL metric'",
  "test('rejects candidate result drift from source oracle'",
  "test('rejects maintenance EAV field cardinality drift'",
  "test('rejects report repetition drift'",
  "test('rejects cross-scale commit mismatch'",
]) {
  if (!comparatorFixture.includes(marker)) {
    fail(`evidence comparator fixture coverage missing ${marker}`);
  }
}

console.log('[verify-index-storage-source-oracle] source oracle and complete evidence metrics are statically guarded');

#!/usr/bin/env node

import { readFileSync } from 'node:fs';

const root = new URL('../../', import.meta.url);
const read = (path) => readFileSync(new URL(path, root), 'utf8');
const fail = (message) => {
  console.error(`[verify-index-storage-source-oracle] ${message}`);
  process.exit(1);
};

const benchmarkModule = read('ops/benches/src/index_storage/mod.rs');
const explainParser = read('ops/benches/src/index_storage/explain.rs');
const sourceSql = read('ops/benches/src/index_storage/sql/source.rs');
const sqlModule = read('ops/benches/src/index_storage/sql/mod.rs');
const runner = read('ops/benches/src/index_storage/runner.rs');
const mutationRunner = read('ops/benches/src/index_storage/mutation_runner.rs');
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

for (const marker of ['mod explain;', 'source_workloads']) {
  if (!benchmarkModule.includes(marker)) fail(`benchmark module missing ${marker}`);
}
for (const marker of [
  'parse_read_explain_metrics',
  'parse_mutation_explain_metrics',
  'root_and_plan_node',
  'required_non_negative_f64',
  'required_non_negative_u64',
  'required_maximum_metric',
  'EXPLAIN result must contain exactly one root entry',
]) {
  if (!explainParser.includes(marker)) fail(`Rust EXPLAIN parser missing ${marker}`);
}

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
  'parse_read_explain_metrics(&plan)',
  'pub planning_time_ms: f64',
  'pub execution_time_ms: f64',
  'pub shared_hit_blocks: u64',
  'pub shared_read_blocks: u64',
  'differs from source oracle',
]) {
  if (!runner.includes(marker)) fail(`read runner missing evidence contract ${marker}`);
}
for (const marker of [
  'parse_mutation_explain_metrics(&plan)',
  'pub planning_time_ms: f64',
  'pub execution_time_ms: f64',
  'pub shared_hit_blocks: u64',
  'pub shared_read_blocks: u64',
  'pub maximum_node_wal_records: u64',
  'pub maximum_node_wal_fpi: u64',
  'pub maximum_node_wal_bytes: u64',
]) {
  if (!mutationRunner.includes(marker)) fail(`mutation runner missing evidence contract ${marker}`);
}
for (const legacy of [
  'pub planning_time_ms: Option<f64>',
  'pub execution_time_ms: Option<f64>',
  'pub shared_hit_blocks: Option<u64>',
  'pub shared_read_blocks: Option<u64>',
  'pub maximum_node_wal_records: Option<u64>',
  'pub maximum_node_wal_fpi: Option<u64>',
  'pub maximum_node_wal_bytes: Option<u64>',
]) {
  if (runner.includes(legacy) || mutationRunner.includes(legacy)) {
    fail(`Rust runner restored nullable required EXPLAIN metric: ${legacy}`);
  }
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

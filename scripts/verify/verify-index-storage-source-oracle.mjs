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

for (const marker of [
  'mod explain;',
  'source_workloads',
  'read_workload_contract',
  'RESULT_DIGEST_CONTRACT',
]) {
  if (!benchmarkModule.includes(marker)) fail(`benchmark module missing ${marker}`);
}
for (const marker of [
  'parse_read_explain_metrics',
  'parse_mutation_explain_metrics',
  'root_and_plan_node',
  'required_non_negative_f64',
  'required_direct_metric_pair',
  'required_maximum_metric_pair',
  'required_maximum_metric_triple',
  'first.unwrap_or(0)',
  'second.unwrap_or(0)',
  'third.unwrap_or(0)',
  'EXPLAIN result must contain exactly one root entry',
  'missing the {family} metric family',
  'omitted_members_of_present_metric_family_become_zero',
  'required_metric_family_fails_closed_when_absent',
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
  'pub const RESULT_DIGEST_CONTRACT: &str = "ordered_length_prefixed_json_v1"',
  'pub fn source_workloads(config: &DatasetConfig) -> Vec<Workload>',
  'source::workloads(&WorkloadContext::new(config))',
  'pub(crate) fn read_workload_contract',
  'digest_order_by',
  'sql_order_marker',
  'assert_read_workload_contract',
  'ORDER BY entity_id LIMIT 100',
  'ORDER BY price_minor, entity_id LIMIT 100',
]) {
  if (!sqlModule.includes(marker)) fail(`SQL module missing read digest contract ${marker}`);
}

for (const marker of [
  'pub result_digest_contract: &\'static str',
  'result_digest_contract: RESULT_DIGEST_CONTRACT',
  'pub source_workloads: Vec<SourceWorkloadReport>',
  'run_source_workloads(&db, &config.dataset)',
  'validate_semantic_parity(&source_workload_reports, &prototypes)',
  'parse_read_explain_metrics(&plan)',
  'read_workload_contract(workload_name).digest_order_by',
  'ORDER BY {order_by}',
  'result_json.len()',
  'SELECT md5($1::text) AS result_digest',
  'pub planning_time_ms: f64',
  'pub execution_time_ms: f64',
  'pub shared_hit_blocks: u64',
  'pub shared_read_blocks: u64',
  'differs from source oracle',
]) {
  if (!runner.includes(marker)) fail(`read runner missing evidence contract ${marker}`);
}
for (const legacy of [
  "string_agg(row_to_json(result)::text, '|'",
  'ORDER BY row_to_json(result)::text',
]) {
  if (runner.includes(legacy)) fail(`read runner restored unordered/set-like digest: ${legacy}`);
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
  "const resultDigestContract = 'ordered_length_prefixed_json_v1'",
  'const readOrderMarkers = new Map',
  'requireReadOrdering',
  'read.result_digest_contract',
  'result_digest_contract: resultDigestContract',
  'read.source_workloads',
  "'source workload order'",
  "sourceWorkload.sql.includes('idx_bench_source.')",
  "workload.sql.includes('idx_bench_source.')",
  'RFC 3339 UTC timestamp',
  'server_version_num must contain only digits',
  'differs from source oracle',
  'source_workload_names: canonicalReadWorkloads',
]) {
  if (!validator.includes(marker)) fail(`packet validator missing strict guard ${marker}`);
}
if (validator.includes('baselineReadWorkloads')) {
  fail('packet validator must not restore first-candidate read parity');
}

for (const marker of [
  "const resultDigestContract = 'ordered_length_prefixed_json_v1'",
  'const readOrderMarkers = new Map',
  'requireReadOrdering',
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
  'same_result_digest_contract',
  'same_dataset_shape',
  'same_source_oracle_shape',
  'result_rows_ratio_1m_to_100k',
  'fail closed on report shape, metrics, plans, effects, ordering, digest semantics, and cardinalities',
  'Result digest contract:',
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
  "test('rejects missing read digest contract'",
  "test('rejects provenance digest contract drift'",
  "test('rejects source workload without canonical ordering'",
  "test('rejects candidate workload without canonical ordering'",
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

console.log('[verify-index-storage-source-oracle] source oracle, self-described ordered digests, and complete evidence metrics are statically guarded');

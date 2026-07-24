import { existsSync, readFileSync } from 'node:fs';

const root = new URL('../../', import.meta.url);
const read = (path) => readFileSync(new URL(path, root), 'utf8');
const exists = (path) => existsSync(new URL(path, root));
const fail = (message) => {
  console.error(`[verify-index-fba] ${message}`);
  process.exit(1);
};

const lib = read('crates/rustok-index/src/lib.rs');
const domain = read('crates/rustok-index/src/domain/mod.rs');
const cargo = read('crates/rustok-index/Cargo.toml');
const productionDependencySections = cargo
  .split(/\n(?=\[)/u)
  .filter((section) => {
    const header = section.split('\n', 1)[0].trim();
    return header === '[dependencies]' || /^\[target\..+\.dependencies\]$/u.test(header);
  })
  .join('\n');
const manifest = read('crates/rustok-index/rustok-module.toml');
const plan = read('crates/rustok-index/docs/implementation-plan.md');
const normalizedPlan = plan.replace(/\s+/gu, ' ');
const benchmarkDoc = read('crates/rustok-index/docs/storage-benchmark.md');
const normalizedBenchmarkDoc = benchmarkDoc.replace(/\s+/gu, ' ');
const benchmarkCargo = read('ops/benches/Cargo.toml');
const benchmarkConfig = read('ops/benches/src/index_storage/config.rs');
const benchmarkConnection = read('ops/benches/src/index_storage/connection.rs');
const smokeWorkflow = read('.github/workflows/index-storage-smoke.yml');
const scaleWorkflow = read('.github/workflows/index-storage-scale-evidence.yml');
const benchmarkSql = [
  'ops/benches/src/index_storage/sql/mod.rs',
  'ops/benches/src/index_storage/sql/source.rs',
  'ops/benches/src/index_storage/sql/common.rs',
  'ops/benches/src/index_storage/sql/maintenance.rs',
  'ops/benches/src/index_storage/sql/jsonb.rs',
  'ops/benches/src/index_storage/sql/eav.rs',
  'ops/benches/src/index_storage/sql/hot.rs',
].map(read).join('\n');
const benchmarkRunner = read('ops/benches/src/index_storage/runner.rs');
const mutationRunner = read('ops/benches/src/index_storage/mutation_runner.rs');
const maintenanceRunner = read('ops/benches/src/index_storage/maintenance_runner.rs');
const serverDispatcher = read('apps/server/src/services/module_event_dispatcher.rs');

for (const obsolete of [
  'crates/rustok-index/src/ports.rs',
  'crates/rustok-index/src/models.rs',
  'crates/rustok-index/src/error.rs',
  'crates/rustok-index/src/traits.rs',
  'crates/rustok-index/src/content',
  'crates/rustok-index/src/product',
  'crates/rustok-index/src/flex',
  'crates/rustok-index/src/search',
  'crates/rustok-index/src/migrations',
  'crates/rustok-index/contracts/index-fba-registry.json',
  'crates/rustok-index/contracts/evidence/index-contract-test-static-matrix.json',
  'crates/rustok-index/contracts/evidence/index-runtime-fallback-smoke.json',
]) {
  if (exists(obsolete)) fail(`obsolete rewrite artifact still exists: ${obsolete}`);
}

for (const marker of ['pub mod domain;', 'pub mod application;', 'pub use domain::*;', 'pub use application::*;']) {
  if (!lib.includes(marker)) fail(`lib.rs missing ${marker}`);
}
for (const marker of ['IndexSchema', 'IndexRecord', 'IndexMutation', 'IndexQuery', 'FilterExpr']) {
  if (!domain.includes(marker)) fail(`domain surface missing ${marker}`);
}
for (const dependency of [
  'rustok-api',
  'rustok-events',
  'rustok-product',
  'rustok-content',
  'rustok-telemetry',
]) {
  if (productionDependencySections.includes(dependency)) {
    fail(`Index core production dependencies must not include ${dependency}`);
  }
}
for (const sourceModule of [
  'pub mod content;',
  'pub mod product;',
  'pub mod flex;',
  'pub mod search;',
  'pub mod migrations;',
  'pub mod traits;',
  'pub mod error;',
]) {
  if (lib.includes(sourceModule)) fail(`legacy module export returned: ${sourceModule}`);
}
for (const runtimeMarker of [
  'IndexerRuntimeConfig',
  'content_indexer',
  'product_indexer',
  'flex_indexer',
  'record_index_reindex_runtime_config',
]) {
  if (serverDispatcher.includes(runtimeMarker)) {
    fail(`server dispatcher still contains legacy Index marker: ${runtimeMarker}`);
  }
}

if (manifest.includes('[fba.provider]')) fail('legacy FBA provider metadata must not return');
if (!plan.includes('- FBA status: `in_progress`')) fail('plan must keep FBA status in_progress during rewrite');
if (!normalizedPlan.includes('Backward compatibility with the rejected implementation is not a goal')) {
  fail('plan must preserve destructive rewrite policy');
}

for (const marker of [
  '- [x] Add deterministic `smoke`, `100k`, and `1m` dataset presets.',
  '- [x] Prototype JSONB entity rows plus typed expression/GIN indexes.',
  '- [x] Prototype normalized typed field-value rows.',
  '- [x] Prototype a specialized hot typed projection as the comparison baseline.',
  '- [x] Verify source/candidate entity and link cardinality before timing.',
  '- [x] Verify identical workload result digests across all candidates.',
  '- [x] Add deterministic Product batch update and delete workloads for all models.',
  '- [x] Isolate every measured mutation in its own rolled-back transaction.',
  '- [x] Add committed update plus delete/reinsert churn cycles for every candidate.',
  '- [x] Execute `VACUUM (ANALYZE)` outside transactions and record its duration.',
  '- [x] Run and archive the `smoke` read, mutation, and maintenance evidence as a',
  '- [x] Run and archive 100k Product-locale row read, mutation, and maintenance',
  '- [ ] Run and archive 1m Product-locale row read, mutation, and maintenance',
  '- [ ] Record the selected model and rejected alternatives in an ADR.',
]) {
  if (!plan.includes(marker)) fail(`M2 plan marker missing: ${marker}`);
}
for (const marker of ['DatasetScale', 'Rows100k', 'Rows1m', 'LocaleKey::new', 'total_link_rows']) {
  if (!benchmarkConfig.includes(marker)) fail(`benchmark config missing ${marker}`);
}
for (const marker of ['min_connections(1)', 'max_connections(1)', 'sqlx_logging(false)']) {
  if (!benchmarkConnection.includes(marker)) fail(`benchmark connection missing ${marker}`);
}
for (const runner of [benchmarkRunner, mutationRunner, maintenanceRunner]) {
  if (!runner.includes('connect_benchmark_database')) {
    fail('every benchmark runner must use the single-session connection helper');
  }
}
for (const marker of [
  'postgres:16',
  'INDEX_BENCH_SCALE: smoke',
  'index-storage-benchmark',
  'index-storage-mutation-benchmark',
  'index-storage-maintenance-benchmark',
  'actions/upload-artifact@v7',
  'retention-days: 90',
]) {
  if (!smokeWorkflow.includes(marker)) fail(`smoke evidence workflow missing ${marker}`);
}
for (const marker of [
  "runner_label: ${{ vars.INDEX_BENCH_LARGE_RUNNER || 'ubuntu-latest' }}",
  'minimum_free_bytes: 35000000000',
  'scale: 1m',
  'needs: contract',
]) {
  if (!scaleWorkflow.includes(marker)) fail(`scale evidence workflow missing ${marker}`);
}
if (scaleWorkflow.includes('evidence-1m-runner-required')) {
  fail('scale workflow must not restore the obsolete unconditional larger-runner failure job');
}
for (const marker of [
  'Prototype::Jsonb',
  'Prototype::TypedEav',
  'Prototype::HotProjection',
  'idx_bench_jsonb',
  'idx_bench_eav',
  'idx_bench_hot',
  'two_hop_channel_filter',
  "product_variant.target_entity = 'variant'",
  "variant_channel.target_entity = 'sales_channel'",
  'keyset_page',
  'update_product_batch',
  'delete_product_batch',
  'churn_cycle_sql',
  'vacuum_statements',
  'VACUUM (ANALYZE)',
  'CREATE TABLE {schema}.link',
]) {
  if (!benchmarkSql.includes(marker)) fail(`benchmark SQL missing ${marker}`);
}
for (const marker of [
  'EXPLAIN (ANALYZE, BUFFERS, WAL, FORMAT JSON)',
  'pg_total_relation_size',
  'validate_cardinality',
  'validate_semantic_parity',
  'result_digest',
  'write_report',
]) {
  if (!benchmarkRunner.includes(marker)) fail(`benchmark runner missing ${marker}`);
}
for (const marker of [
  'TransactionTrait',
  'transaction.rollback().await',
  'affected_entities',
  'affected_links',
  'maximum_node_wal_records',
  'maximum_node_wal_fpi',
  'maximum_node_wal_bytes',
  'EXPLAIN (ANALYZE, BUFFERS, WAL, FORMAT JSON)',
  'write_mutation_report',
]) {
  if (!mutationRunner.includes(marker)) fail(`mutation runner missing ${marker}`);
}
for (const marker of [
  'churn_cycle_sql',
  'transaction.commit().await',
  'for statement in vacuum_statements(prototype)',
  'pg_stat_force_next_flush',
  'pg_stat_user_tables',
  'estimated_dead_tuples',
  'schema_bytes',
  'vacuum_duration_ms',
  'write_maintenance_report',
]) {
  if (!maintenanceRunner.includes(marker) && !benchmarkSql.includes(marker)) {
    fail(`maintenance benchmark missing ${marker}`);
  }
}
if (maintenanceRunner.includes('execute_unprepared(&vacuum_sql')) {
  fail('VACUUM statements must not be combined into one implicit transaction');
}
for (const binary of [
  'index-storage-benchmark',
  'index-storage-mutation-benchmark',
  'index-storage-maintenance-benchmark',
]) {
  if (!benchmarkCargo.includes(`name = "${binary}"`)) {
    fail(`benchmark executable is not registered: ${binary}`);
  }
}
if (!normalizedBenchmarkDoc.includes('Production migrations: intentionally absent')) {
  fail('benchmark documentation must preserve the production-migration boundary');
}
if (!normalizedBenchmarkDoc.includes('It does not run `VACUUM FULL`')) {
  fail('benchmark documentation must reject VACUUM FULL as a health assumption');
}
if (!normalizedBenchmarkDoc.includes('Smoke evidence: archived from Actions run `30041091121`')) {
  fail('benchmark documentation must record the inspected smoke evidence run');
}
if (!normalizedBenchmarkDoc.includes('100k evidence: archived and inspected from Actions run `30051321255`')) {
  fail('benchmark documentation must record the inspected 100k evidence run');
}
if (!normalizedBenchmarkDoc.includes('1m evidence: enabled on `INDEX_BENCH_LARGE_RUNNER` when configured, otherwise `ubuntu-latest`, with a fail-closed 35 GB free-disk check')) {
  fail('benchmark documentation must keep the guarded 1m runner policy visible');
}

console.log('[verify-index-fba] Index core boundary and M2 benchmark/evidence state are consistent');

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
const sqlModule = read('ops/benches/src/index_storage/sql/mod.rs');
const commonSql = read('ops/benches/src/index_storage/sql/common.rs');
const maintenanceSql = read('ops/benches/src/index_storage/sql/maintenance.rs');
const jsonbSql = read('ops/benches/src/index_storage/sql/jsonb.rs');
const eavSql = read('ops/benches/src/index_storage/sql/eav.rs');
const hotSql = read('ops/benches/src/index_storage/sql/hot.rs');
const normalizedCommonSql = commonSql.replace(/\s+/gu, ' ');
const normalizedMaintenanceSql = maintenanceSql.replace(/\s+/gu, ' ');
const normalizedJsonbSql = jsonbSql.replace(/\s+/gu, ' ');
const normalizedEavSql = eavSql.replace(/\s+/gu, ' ');
const normalizedHotSql = hotSql.replace(/\s+/gu, ' ');
const benchmarkSql = [
  sqlModule,
  'ops/benches/src/index_storage/sql/source.rs',
  commonSql,
  maintenanceSql,
  jsonbSql,
  eavSql,
  hotSql,
].map((entry) => entry.includes('\n') ? entry : read(entry)).join('\n');
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
  '- [x] Preserve complete module/entity/schema-version identity in typed EAV field',
  '- [x] Scope JSONB and typed EAV maintenance entity mutations by the complete schema',
  '- [x] Add static guards for full-identity EAV and maintenance SQL.',
  '- [x] Prototype a specialized hot typed projection as the comparison baseline.',
  '- [x] Verify source/candidate entity and link cardinality before timing.',
  '- [x] Verify identical workload result digests across all candidates.',
  '- [x] Add deterministic Product batch update and delete workloads for all models.',
  '- [x] Isolate every measured mutation in its own rolled-back transaction.',
  '- [x] Add committed update plus delete/reinsert churn cycles for every candidate.',
  '- [x] Execute `VACUUM (ANALYZE)` outside transactions and record its duration.',
  '- [x] Run and archive the `smoke` read, mutation, and maintenance evidence as a',
  '- [x] Record the candidate operational review and ADR completion checklist.',
  '- [ ] Run and archive replacement 100k Product-locale row read, mutation, and',
  '- [ ] Run and archive replacement 1m Product-locale row read, mutation, and',
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
  'source_module text NOT NULL',
  'source_schema_version integer NOT NULL CHECK (source_schema_version > 0)',
  'target_module text NOT NULL',
  'target_schema_version integer NOT NULL CHECK (target_schema_version > 0)',
  'tenant_id, source_module, source_entity, source_schema_version, source_entity_id',
  'tenant_id, target_module, target_entity, target_schema_version, target_entity_id',
]) {
  if (!normalizedCommonSql.includes(marker)) fail(`benchmark link envelope missing ${marker}`);
}
for (const marker of [
  'assert_full_link_identity_sql',
  'workload.sql = common::assert_full_link_identity_sql(workload.sql)',
  'common::assert_full_link_identity_sql(maintenance::churn_cycle_sql',
]) {
  if (!sqlModule.includes(marker)) fail(`benchmark SQL identity guard missing ${marker}`);
}
for (const [label, source] of [
  ['JSONB', normalizedJsonbSql],
  ['typed EAV', normalizedEavSql],
  ['hot projection', normalizedHotSql],
]) {
  for (const marker of [
    'product_variant.source_module',
    'product_variant.source_schema_version',
    'product_variant.target_module',
    'product_variant.target_schema_version',
    'variant_channel.source_module = product_variant.target_module',
    'variant_channel.source_schema_version = product_variant.target_schema_version',
    'variant_channel.target_module',
    'variant_channel.target_schema_version',
    "link.source_module = 'product'",
    'link.source_schema_version = 1',
  ]) {
    if (!source.includes(marker)) fail(`${label} link identity missing ${marker}`);
  }
}
for (const marker of [
  'source_module, source_entity, source_schema_version',
  'target_module, target_entity, target_schema_version',
  "link.source_module = 'product'",
  'link.source_schema_version = 1',
]) {
  if (!normalizedMaintenanceSql.includes(marker)) {
    fail(`maintenance link identity missing ${marker}`);
  }
}
for (const legacy of [
  'tenant_id, source_entity, source_entity_id, source_locale, link_name, ordinal, target_entity, target_entity_id, target_locale',
  "product_variant.source_entity = 'product' AND product_variant.source_entity_id",
  "variant_channel.source_entity = 'variant' AND variant_channel.source_entity_id",
  "link.source_entity = 'product' AND link.source_locale",
]) {
  if ([normalizedCommonSql, normalizedJsonbSql, normalizedEavSql, normalizedHotSql, normalizedMaintenanceSql]
    .some((source) => source.includes(legacy))) {
    fail(`incomplete benchmark link identity returned: ${legacy}`);
  }
}
for (const marker of [
  'CREATE TABLE idx_bench_eav.field_value ( tenant_id uuid NOT NULL, module_name text NOT NULL, entity_name text NOT NULL, schema_version integer NOT NULL',
  'PRIMARY KEY ( tenant_id, module_name, entity_name, schema_version, entity_id, locale, field_name, ordinal )',
  'FOREIGN KEY ( tenant_id, module_name, entity_name, schema_version, entity_id, locale ) REFERENCES idx_bench_eav.entity',
  'status.module_name = entity.module_name',
  'status.schema_version = entity.schema_version',
  'channel_code.module_name = variant_channel.target_module',
  'channel_code.schema_version = variant_channel.target_schema_version',
  "field.module_name = 'product'",
  'field.schema_version = 1',
]) {
  if (!normalizedEavSql.includes(marker)) fail(`typed EAV full-identity guard missing: ${marker}`);
}
for (const marker of [
  "entity.module_name = 'product' AND entity.entity_name = 'product' AND entity.schema_version = 1",
  "field.module_name = 'product' AND field.entity_name = 'product' AND field.schema_version = 1",
  'tenant_id, module_name, entity_name, schema_version, entity_id, locale, field_name',
]) {
  if (!normalizedMaintenanceSql.includes(marker)) {
    fail(`maintenance full-identity guard missing: ${marker}`);
  }
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
if (!normalizedBenchmarkDoc.includes('Smoke evidence: historical harness-sanity packet from Actions run `30041091121`')) {
  fail('benchmark documentation must record the historical smoke evidence run');
}
if (!normalizedBenchmarkDoc.includes('100k evidence: historical diagnostic packet from Actions run `30051321255`')) {
  fail('benchmark documentation must record the historical 100k evidence run');
}
if (!normalizedBenchmarkDoc.includes('Replacement evidence: same-commit 100k and 1m packets pending after full-identity corrections')) {
  fail('benchmark documentation must keep replacement scale evidence pending');
}
if (!normalizedBenchmarkDoc.includes('1m evidence: enabled on `INDEX_BENCH_LARGE_RUNNER` when configured, otherwise `ubuntu-latest`, with a fail-closed 35 GB free-disk check')) {
  fail('benchmark documentation must keep the guarded 1m runner policy visible');
}

console.log('[verify-index-fba] Index core boundary and M2 benchmark/evidence state are consistent');

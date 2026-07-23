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
const manifest = read('crates/rustok-index/rustok-module.toml');
const plan = read('crates/rustok-index/docs/implementation-plan.md');
const benchmarkDoc = read('crates/rustok-index/docs/storage-benchmark.md');
const benchmarkCargo = read('ops/benches/Cargo.toml');
const benchmarkConfig = read('ops/benches/src/index_storage/config.rs');
const benchmarkSql = read('ops/benches/src/index_storage/sql.rs');
const benchmarkRunner = read('ops/benches/src/index_storage/runner.rs');
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
  if (cargo.includes(dependency)) fail(`Index core must not depend on ${dependency}`);
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
if (!plan.includes('Backward compatibility with the rejected implementation is not a goal')) {
  fail('plan must preserve destructive rewrite policy');
}

for (const marker of [
  '- [x] Add deterministic `smoke`, `100k`, and `1m` dataset presets.',
  '- [x] Prototype JSONB entity rows plus typed expression/GIN indexes.',
  '- [x] Prototype normalized typed field-value rows.',
  '- [x] Prototype a specialized hot typed projection as the comparison baseline.',
  '- [ ] Run and archive 100k Product-locale row evidence.',
  '- [ ] Record the selected model and rejected alternatives in an ADR.',
]) {
  if (!plan.includes(marker)) fail(`M2 plan marker missing: ${marker}`);
}
for (const marker of ['DatasetScale', 'Rows100k', 'Rows1m', 'LocaleKey::new']) {
  if (!benchmarkConfig.includes(marker)) fail(`benchmark config missing ${marker}`);
}
for (const marker of [
  'Prototype::Jsonb',
  'Prototype::TypedEav',
  'Prototype::HotProjection',
  'idx_bench_jsonb',
  'idx_bench_eav',
  'idx_bench_hot',
  'two_hop_channel_filter',
  'keyset_page',
  'CREATE TABLE {schema}.link',
]) {
  if (!benchmarkSql.includes(marker)) fail(`benchmark SQL missing ${marker}`);
}
for (const marker of [
  'EXPLAIN (ANALYZE, BUFFERS, WAL, FORMAT JSON)',
  'pg_total_relation_size',
  'write_report',
]) {
  if (!benchmarkRunner.includes(marker)) fail(`benchmark runner missing ${marker}`);
}
if (!benchmarkCargo.includes('name = "index-storage-benchmark"')) {
  fail('benchmark executable is not registered');
}
if (!benchmarkDoc.includes('Production migrations: intentionally absent')) {
  fail('benchmark documentation must preserve the production-migration boundary');
}

console.log('[verify-index-fba] Index core boundary and M2 benchmark separation are consistent');

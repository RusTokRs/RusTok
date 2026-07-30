#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const read = (relative) => fs.readFileSync(path.join(root, relative), 'utf8');
const fail = (message) => {
  console.error(`[verify-index-product-source] ${message}`);
  process.exit(1);
};
const requireMarkers = (relative, markers) => {
  const source = read(relative);
  for (const marker of markers) {
    if (!source.includes(marker)) fail(`${relative} is missing ${marker}`);
  }
  return source;
};
const forbidMarkers = (relative, source, markers) => {
  for (const marker of markers) {
    if (source.includes(marker)) fail(`${relative} contains forbidden marker ${marker}`);
  }
};

const factoryPath = 'crates/rustok-index/src/infrastructure/postgres/source_factory.rs';
const factory = requireMarkers(factoryPath, [
  'pub trait PostgresIndexSourceFactory',
  'pub struct PostgresIndexSourceFactoryCatalog',
  'pub enum PostgresIndexSourceFactoryError',
  'pub fn register_postgres_index_source_factory',
  'pub fn materialize_postgres_index_sources',
  'let mut staged = extensions.clone();',
  '*extensions = staged;',
  'failing_factory_does_not_commit_partial_source_catalog',
  'duplicate_materialization_fails_closed',
]);
forbidMarkers(factoryPath, factory, [
  '.query_one(',
  '.query_all(',
  '.execute(',
  '.begin()',
  'tokio::spawn',
  'tokio::time::sleep',
  'loop {',
  'rustok_product',
]);

requireMarkers('crates/rustok-index/src/lib.rs', [
  'PostgresIndexSourceFactoryCatalog',
  'materialize_postgres_index_sources',
  'register_postgres_index_source_factory',
  'extensions.get_or_insert_with::<PostgresIndexSourceFactoryCatalog',
]);
requireMarkers('crates/rustok-index/src/infrastructure/postgres/mod.rs', [
  'mod source_factory;',
  'PostgresIndexSourceFactory',
  'PostgresIndexSourceFactoryCatalog',
  'materialize_postgres_index_sources',
]);

const productCargo = requireMarkers('crates/rustok-product/Cargo.toml', [
  'index = ["dep:rustok-index", "dep:sha2"]',
  'rustok-index = { workspace = true, optional = true }',
  'sha2 = { workspace = true, optional = true }',
]);
forbidMarkers('crates/rustok-product/Cargo.toml', productCargo, [
  'rustok-search =',
]);

const productRootPath = 'crates/rustok-product/src/lib.rs';
const productRoot = requireMarkers(productRootPath, [
  '#[cfg(feature = "index")]',
  'pub mod index;',
  '&["taxonomy", "index"]',
  'register_index_schema_source(extensions, self.slug(), schema)',
  'register_postgres_index_source_factory(',
  'ProductPostgresIndexSourceFactory',
]);
forbidMarkers(productRootPath, productRoot, [
  'PostgresMutationStore',
  'PostgresIndexReplayRunner',
]);

const sourcePath = 'crates/rustok-product/src/index.rs';
const source = requireMarkers(sourcePath, [
  'pub const PRODUCT_INDEX_MODULE: &str = "rustok-product";',
  'pub const PRODUCT_INDEX_ENTITY: &str = "product";',
  'pub const PRODUCT_INDEX_SOURCE: &str = "product-postgres-primary";',
  'locale_mode: LocaleMode::Required',
  'pub struct ProductPostgresIndexSourceFactory',
  'impl PostgresIndexSourceFactory for ProductPostgresIndexSourceFactory',
  'pub struct ProductPostgresIndexSource',
  'impl IndexSource for ProductPostgresIndexSource',
  'p.tenant_id,',
  'p.index_revision,',
  '(p.id, t.locale) > ($2, $3)',
  'ORDER BY p.id ASC, t.locale ASC',
  'request.limit() + 1',
  'WITH requested(product_id, locale) AS (VALUES {})',
  'JOIN requested r ON r.product_id = p.id AND r.locale = t.locale',
  'rustok-product-index-replay-event-v1',
  'product_index_storage_unavailable',
  'product_index_backend_unsupported',
  'product_schema_is_locale_required_and_scalar_only',
  'replay_event_identity_is_stable_and_revision_sensitive',
  'cursor_rejects_nil_product_and_noncanonical_locale',
]);
forbidMarkers(sourcePath, source, [
  'index_entities',
  'index_links',
  'index_jobs',
  'index_checkpoints',
  'SELECT *',
  'ORDER BY p.index_revision',
  '(p.index_revision, p.id, t.locale)',
  'tokio::spawn',
  'tokio::time::sleep',
  'loop {',
  'rustok_search',
]);

const migrationPath =
  'crates/rustok-product/src/migrations/m20260730_000001_add_product_index_revision.rs';
const migration = requireMarkers(migrationPath, [
  'ADD COLUMN index_revision BIGINT NOT NULL DEFAULT 1',
  'chk_products_index_revision_positive',
  'trg_products_bump_index_revision',
  'trg_product_translations_bump_index_revision',
  'AFTER INSERT OR UPDATE OR DELETE ON product_translations',
]);
forbidMarkers(migrationPath, migration, [
  'idx_products_index_replay',
  'index_entities',
  'index_links',
  'index_jobs',
  'index_checkpoints',
]);
requireMarkers(
  'crates/rustok-product/src/migrations/m20260701_000002_add_product_catalog_tenant_consistency_constraints.rs',
  ['UNIQUE (tenant_id, id)'],
);
requireMarkers(
  'crates/rustok-product/src/migrations/m20250130_000012_create_commerce_products.rs',
  [
    '.name("idx_product_trans_unique")',
    '.col(ProductTranslations::ProductId)',
    '.col(ProductTranslations::Locale)',
  ],
);
requireMarkers('crates/rustok-product/src/migrations/mod.rs', [
  'mod m20260730_000001_add_product_index_revision;',
  'Box::new(m20260730_000001_add_product_index_revision::Migration)',
]);

requireMarkers('crates/rustok-distribution/Cargo.toml', [
  'mod-product = ["dep:rustok-product", "rustok-product/index", "mod-taxonomy"]',
]);
const serverPath = 'apps/server/src/services/index_replay_runtime_composition.rs';
const server = requireMarkers(serverPath, [
  'materialize_postgres_index_sources(extensions, db.clone())',
  'materialize_index_source_registry(extensions)',
  'materialize_postgres_index_replay_runtime(extensions, db)',
]);
const factoryMaterialization = server.indexOf(
  'materialize_postgres_index_sources(extensions, db.clone())',
);
const registryMaterialization = server.indexOf(
  'materialize_index_source_registry(extensions)',
  factoryMaterialization,
);
const runtimeMaterialization = server.indexOf(
  'materialize_postgres_index_replay_runtime(extensions, db)',
  registryMaterialization,
);
if (
  factoryMaterialization < 0
  || registryMaterialization <= factoryMaterialization
  || runtimeMaterialization <= registryMaterialization
) {
  fail('server must construct source adapters before freezing the registry and replay runtime');
}
forbidMarkers(serverPath, server, ['rustok_product', 'ProductPostgresIndexSource']);

requireMarkers('crates/rustok-product/src/contract_tests.rs', [
  'product_publishes_index_schema_and_postgres_source_factory',
  'PostgresIndexSourceFactoryCatalog',
]);
requireMarkers('crates/rustok-product/tests/module.rs', [
  '#[cfg(feature = "index")]',
  '&["taxonomy", "index"]',
]);
requireMarkers('crates/rustok-index/docs/m7-product-source.md', [
  'Status: `source_complete_owner_execution_pending`',
  '`rustok-product::product@1`',
  '`ProductPostgresIndexSource`',
  'stable `(product_id, locale)` identity',
  'it is not the scan cursor.',
  'Product hard deletes do not yet emit durable Index tombstones.',
  'Runtime capability presence does not establish persisted schema readiness.',
  'maintainer-run',
]);
requireMarkers('crates/rustok-product/README.md', [
  'publish one owner-generic locale-required',
  '`index_revision`',
  '`(product_id, locale)`',
  'Hard-delete tombstones',
]);
requireMarkers('scripts/verify/verify-index-query-contract.mjs', [
  "'verify-index-product-source.mjs'",
]);

console.log('[verify-index-product-source] OK');

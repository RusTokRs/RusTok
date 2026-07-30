#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const resolve = (relative) => path.join(root, relative);
const read = (relative) => fs.readFileSync(resolve(relative), 'utf8');
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

const eventIdPath = 'crates/rustok-index/src/application/source_event_id.rs';
const eventId = requireMarkers(eventIdPath, [
  'pub fn derive_index_source_event_id(',
  'rustok-index-source-event-id-v1',
  'IndexSourceEventIdError::NilTenantId',
  'IndexSourceEventIdError::NilEntityId',
  'IndexSourceEventIdError::ZeroSourceVersion',
  'source_event_identity_is_stable_and_scope_sensitive',
]);
forbidMarkers(eventIdPath, eventId, ['rand::', 'Uuid::new_v4', 'tokio::']);
requireMarkers('crates/rustok-index/src/application/mod.rs', [
  'mod source_event_id;',
  'IndexSourceEventIdError',
  'derive_index_source_event_id',
]);
requireMarkers('crates/rustok-index/src/lib.rs', [
  'PostgresIndexSourceFactoryCatalog',
  'materialize_postgres_index_sources',
  'register_postgres_index_source_factory',
  'extensions.get_or_insert_with::<PostgresIndexSourceFactoryCatalog',
]);

const productCargo = read('crates/rustok-product/Cargo.toml');
forbidMarkers('crates/rustok-product/Cargo.toml', productCargo, [
  'rustok-index',
  'dep:sha2',
  '[features]',
]);
const productRootPath = 'crates/rustok-product/src/lib.rs';
const productRoot = requireMarkers(productRootPath, [
  'pub struct ProductRuntimeSelected;',
  'extensions.insert(ProductRuntimeSelected);',
  '&["taxonomy"]',
]);
forbidMarkers(productRootPath, productRoot, [
  'rustok_index',
  'register_index_schema_source',
  'PostgresIndexSourceFactory',
]);
if (fs.existsSync(resolve('crates/rustok-product/src/index.rs'))) {
  fail('Product owner crate must not contain the selected cross-module Index adapter');
}

const distributionCargo = requireMarkers('crates/rustok-distribution/Cargo.toml', [
  'mod-product = ["dep:rustok-product", "mod-taxonomy"]',
  'async-trait.workspace = true',
  'sea-orm.workspace = true',
]);
forbidMarkers('crates/rustok-distribution/Cargo.toml', distributionCargo, [
  'rustok-product/index',
]);
const distributionRoot = requireMarkers('crates/rustok-distribution/src/lib.rs', [
  '#[cfg(feature = "mod-product")]',
  'mod product_index;',
  'register_selected_index_bridges(&mut extensions)?;',
  'product_index::register(extensions)?;',
  'selected_product_bridge_publishes_schema_and_source_factory',
]);
const bridgeRegistration = distributionRoot.indexOf(
  'register_selected_index_bridges(&mut extensions)?;',
);
const schemaMaterialization = distributionRoot.indexOf(
  'materialize_index_schema_sources(&mut extensions)?;',
  bridgeRegistration,
);
if (bridgeRegistration < 0 || schemaMaterialization <= bridgeRegistration) {
  fail('selected Index bridges must register before immutable schema materialization');
}

const sourcePath = 'crates/rustok-distribution/src/product_index.rs';
const source = requireMarkers(sourcePath, [
  'PRODUCT_INDEX_SOURCE: &str = "product-postgres-primary"',
  'PRODUCT_EVENT_DOMAIN: &str = "rustok-product.product-replay-v1"',
  'extensions.contains::<rustok_product::ProductRuntimeSelected>()',
  'register_index_schema_source(extensions, "product", schema)',
  'register_postgres_index_source_factory(',
  'locale_mode: LocaleMode::Required',
  'impl PostgresIndexSourceFactory for ProductPostgresIndexSourceFactory',
  'impl IndexSource for ProductPostgresIndexSource',
  'p.tenant_id,',
  'p.index_revision,',
  '(p.id, t.locale) > ($2, $3)',
  'ORDER BY p.id ASC, t.locale ASC',
  'request.limit() + 1',
  'WITH requested(product_id, locale) AS (VALUES {})',
  'JOIN requested r ON r.product_id = p.id AND r.locale = t.locale',
  'derive_index_source_event_id(',
  'product_index_storage_unavailable',
  'product_index_backend_unsupported',
  '#[serde(deny_unknown_fields)]',
  'if locale.as_str() != raw_locale',
  'selected_product_bridge_skips_partial_registry_without_product_module',
  'selected_product_bridge_registers_schema_and_factory',
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
  'OLD.index_revision = 9223372036854775807',
  'NEW.index_revision := OLD.index_revision + 1;',
  'trg_products_bump_index_revision',
  'trg_product_translations_bump_index_revision',
  'AFTER INSERT OR UPDATE OR DELETE ON product_translations',
]);
forbidMarkers(migrationPath, migration, [
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
  'product_module_publishes_only_a_typed_selection_marker_for_cross_module_bridges',
  'assert!(!cargo.contains("rustok-index"));',
]);
requireMarkers('crates/rustok-product/tests/module.rs', [
  'assert_eq!(module.dependencies(), &["taxonomy"]);',
]);
requireMarkers('crates/rustok-index/docs/m7-product-source.md', [
  'Status: `source_complete_owner_execution_pending`',
  '`rustok-product::product@1`',
  '`rustok-distribution` is the selected cross-module bridge.',
  'stable `(product_id, locale)` identity',
  'it is not the scan cursor.',
  'Product hard deletes do not yet emit durable Index tombstones.',
  'Runtime capability presence does not establish persisted schema readiness.',
  'maintainer-run',
]);
requireMarkers('crates/rustok-product/README.md', [
  '`ProductRuntimeSelected`',
  '`index_revision`',
  '`(product_id, locale)`',
  'does not depend on `rustok-index`',
]);
requireMarkers('scripts/verify/verify-index-query-contract.mjs', [
  "'verify-index-product-source.mjs'",
]);

console.log('[verify-index-product-source] OK');

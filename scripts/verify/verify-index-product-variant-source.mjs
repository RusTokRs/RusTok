#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const resolve = (relative) => path.join(root, relative);
const read = (relative) => fs.readFileSync(resolve(relative), 'utf8');
const fail = (message) => {
  console.error(`[verify-index-product-variant-source] ${message}`);
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

const modulePath = 'crates/rustok-distribution/src/product_index/mod.rs';
const moduleSource = requireMarkers(modulePath, [
  'mod product;',
  '#[path = "../product_variant_index.rs"]',
  'mod variant;',
  'product::register(extensions)?;',
  'variant::register(extensions)',
  'selected_product_bridge_set_registers_two_schemas_and_two_factories',
  'assert_eq!(factories.len(), 2);',
]);
forbidMarkers(modulePath, moduleSource, ['tokio::spawn', 'tokio::time::sleep', 'loop {']);

const sourcePath = 'crates/rustok-distribution/src/product_variant_index.rs';
const source = requireMarkers(sourcePath, [
  'PRODUCT_VARIANT_INDEX_SOURCE: &str = "product-variant-postgres-primary"',
  'PRODUCT_VARIANT_EVENT_DOMAIN: &str = "rustok-product.product-variant-replay-v1"',
  'extensions.contains::<rustok_product::ProductRuntimeSelected>()',
  'register_index_schema_source(extensions, "product", schema)',
  'register_postgres_index_source_factory(',
  'entity: EntityName::new("product_variant")?',
  'locale_mode: LocaleMode::None',
  'field("product_id", IndexValueType::Uuid, false, true, true)?',
  'impl PostgresIndexSourceFactory for ProductVariantPostgresIndexSourceFactory',
  'impl IndexSource for ProductVariantPostgresIndexSource',
  'FROM product_variants v',
  'v.index_revision,',
  'v.id > $2',
  'ORDER BY v.id ASC',
  'request.limit() + 1',
  'WITH requested(variant_id) AS (VALUES {})',
  'JOIN requested r ON r.variant_id = v.id',
  'product_variant_index_locale_forbidden',
  'derive_index_source_event_id(',
  'locale: None',
  'links: Vec::new()',
  '#[serde(deny_unknown_fields)]',
  'selected_product_variant_schema_is_nonlocalized_and_link_free',
  'selected_product_variant_cursor_rejects_nil_and_unknown_fields',
  'selected_product_variant_bridge_registers_schema_and_factory',
]);
forbidMarkers(sourcePath, source, [
  'product_variant_translations',
  'IndexLink',
  'IndexLinkValue',
  'LocaleKey',
  'ORDER BY v.index_revision',
  '(v.index_revision, v.id)',
  'SELECT *',
  'index_entities',
  'index_links',
  'index_jobs',
  'index_checkpoints',
  'tokio::spawn',
  'tokio::time::sleep',
  'loop {',
  'rustok_search',
]);

const migrationPath =
  'crates/rustok-product/src/migrations/m20260730_000002_add_product_variant_index_revision.rs';
const migration = requireMarkers(migrationPath, [
  'ALTER TABLE product_variants',
  'ADD COLUMN index_revision BIGINT NOT NULL DEFAULT 1',
  'chk_product_variants_index_revision_positive',
  'OLD.index_revision = 9223372036854775807',
  'NEW.index_revision := OLD.index_revision + 1;',
  'trg_product_variants_bump_index_revision',
  'BEFORE UPDATE ON product_variants',
]);
forbidMarkers(migrationPath, migration, [
  'product_variant_translations',
  'index_entities',
  'index_links',
  'index_jobs',
  'index_checkpoints',
]);
requireMarkers('crates/rustok-product/src/migrations/mod.rs', [
  'mod m20260730_000002_add_product_variant_index_revision;',
  'Box::new(m20260730_000002_add_product_variant_index_revision::Migration)',
]);
requireMarkers(
  'crates/rustok-product/src/migrations/m20260701_000002_add_product_catalog_tenant_consistency_constraints.rs',
  ['uq_product_variants_tenant_id', 'UNIQUE (tenant_id, id)'],
);

const productSource = read('crates/rustok-distribution/src/product_index/product.rs');
forbidMarkers('crates/rustok-distribution/src/product_index/product.rs', productSource, [
  'product_variant',
  'PRODUCT_VARIANT_INDEX_SOURCE',
]);

requireMarkers('crates/rustok-index/docs/m7-product-variant-source.md', [
  'Status: `source_complete_owner_execution_pending`',
  '`rustok-product::product_variant@1`',
  'cursor scans ordered by stable `variant_id`',
  'The already published `rustok-product::product@1` schema is unchanged.',
  'would change the Product v1 fingerprint',
  'ProductVariant hard deletes do not yet emit durable Index tombstones.',
  'maintainer-run',
]);
requireMarkers('scripts/verify/verify-index-query-contract.mjs', [
  "'verify-index-product-variant-source.mjs'",
]);

console.log('[verify-index-product-variant-source] OK');

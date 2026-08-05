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
  'mod absence;',
  'pub(crate) mod graph;',
  'mod product;',
  '#[path = "../product_variant_index.rs"]',
  'mod variant;',
  'product::register(extensions)?;',
  'variant::register(extensions)?;',
  'absence::register(extensions)',
  'selected_product_bridge_set_registers_four_schemas_and_three_stable_factories',
  'assert_eq!(factories.len(), 3);',
]);
forbidMarkers(modulePath, moduleSource, ['tokio::spawn', 'tokio::time::sleep', 'loop {']);

const wrapperPath = 'crates/rustok-distribution/src/product_variant_index.rs';
const wrapper = requireMarkers(wrapperPath, [
  'pub(crate) use crate::product_index::graph::PRODUCT_VARIANT_INDEX_SOURCE;',
  'crate::product_index::graph::register_variant(extensions)',
]);
forbidMarkers(wrapperPath, wrapper, ['DatabaseConnection', 'FROM product_variants']);

const sourcePath = 'crates/rustok-distribution/src/product_index/graph.rs';
const source = requireMarkers(sourcePath, [
  'PRODUCT_VARIANT_INDEX_SOURCE: &str = "product-variant-postgres-primary"',
  'PRODUCT_VARIANT_EVENT_DOMAIN_V1: &str = "rustok-product.product-variant-replay-v1"',
  'fn product_variant_v1_schema()',
  'fn product_variant_v2_schema()',
  'reference: product_variant_schema_ref(1)?',
  'reference: product_variant_schema_ref(2)?',
  'locale_mode: LocaleMode::None',
  'fields: product_variant_fields(false)?',
  'fields: product_variant_fields(true)?',
  'scalar_field("product_id", IndexValueType::Uuid, false, true, true)?',
  'scalar_field("sku", IndexValueType::String, true, true, true)?',
  'product_variant_schema_ref(1).map_err(|error| error.to_string())?',
  'product_variant_schema_ref(2).map_err(|error| error.to_string())?',
  'ProductVariantPostgresIndexSource { db }',
  'impl IndexSource for ProductVariantPostgresIndexSource',
  'FROM product_variants v',
  'FROM product_variant_index_tombstones tombstone',
  'v.index_revision,',
  'row.variant_id > $2',
  'ORDER BY row.variant_id ASC',
  'request.limit() + 1',
  'WITH requested(variant_id) AS (VALUES {})',
  'product_variant_index_locale_forbidden',
  'PRODUCT_VARIANT_EVENT_DOMAIN_V1',
  'schema: product_variant_schema_ref(schema_version)?',
  'locale: None',
  'links: Vec::new()',
  '#[serde(deny_unknown_fields)]',
]);
forbidMarkers(sourcePath, source, [
  'product_variant_translations',
  'ORDER BY v.index_revision',
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
requireMarkers(migrationPath, [
  'ALTER TABLE product_variants',
  'ADD COLUMN index_revision BIGINT NOT NULL DEFAULT 1',
  'NEW.index_revision := OLD.index_revision + 1;',
  'trg_product_variants_bump_index_revision',
  'BEFORE UPDATE ON product_variants',
]);
const tombstoneMigration = requireMarkers(
  'crates/rustok-product/src/migrations/m20260731_000004_add_product_index_tombstones.rs',
  [
    'CREATE TABLE product_variant_index_tombstones',
    'rustok_product_variant_capture_index_tombstone',
    'trg_product_variants_capture_index_tombstone',
    'rustok_product_variant_seed_index_revision_from_tombstone',
    'rustok_product_variant_clear_superseded_index_tombstone',
  ],
);
forbidMarkers(
  'crates/rustok-product/src/migrations/m20260731_000004_add_product_index_tombstones.rs',
  tombstoneMigration,
  ['index_entities', 'index_links', 'index_jobs'],
);

requireMarkers('crates/rustok-index/docs/m7-product-variant-source.md', [
  'Status: `source_complete_owner_execution_pending`',
  '`rustok-product::product_variant@1` and `@2`',
  'cursor scans ordered by stable `variant_id`',
  '`product_variant_index_tombstones`',
  'Hard delete stores a tombstone',
  'No ProductVariant locale-absence provider is required',
  'maintainer-run',
]);
requireMarkers('scripts/verify/verify-index-query-contract.mjs', [
  "'verify-index-product-variant-source.mjs'",
]);

console.log('[verify-index-product-variant-source] OK');

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
  'variant::register(extensions)?;',
  'selected_product_bridge_registers_two_current_schemas_three_factories_and_entity_admissions',
]);
forbidMarkers(modulePath, moduleSource, ['mod graph;', 'graph::', 'four_schemas']);

const sourcePath = 'crates/rustok-distribution/src/product_variant_index.rs';
const source = requireMarkers(sourcePath, [
  'PRODUCT_VARIANT_INDEX_SOURCE: &str = "product-variant-postgres-primary"',
  'PRODUCT_VARIANT_EVENT_DOMAIN: &str = "rustok-product.product-variant-replay"',
  'fn product_variant_schema()',
  'locale_mode: LocaleMode::None',
  'scalar_field("id", IndexValueType::Uuid, false, true, true)?',
  'scalar_field("product_id", IndexValueType::Uuid, false, true, true)?',
  'scalar_field("sku", IndexValueType::String, true, true, true)?',
  '[product_variant_schema_ref().map_err(|error| error.to_string())?]',
  'ProductVariantPostgresIndexSource { db }',
  'impl IndexSource for ProductVariantPostgresIndexSource',
  'FROM product_variants v',
  'FROM product_variant_index_tombstones tombstone',
  'row.variant_id > $2',
  'ORDER BY row.variant_id ASC',
  'WITH requested(variant_id) AS (VALUES {})',
  'product_variant_index_locale_forbidden',
  'schema: product_variant_schema_ref()?',
  'locale: None',
  'links: Vec::new()',
  'canonical_product_variant_schema_contains_identity_once',
  'canonical_product_variant_registration_publishes_one_schema_and_one_factory',
]);
forbidMarkers(sourcePath, source, [
  'ProductVariantSchemaVersion',
  'product_variant_v1_schema',
  'product_variant_v2_schema',
  'PRODUCT_VARIANT_EVENT_DOMAIN_V1',
  'PRODUCT_VARIANT_EVENT_DOMAIN_V2',
  'product-variant-replay-v1',
  'product-variant-replay-v2',
  'product_variant_schema_ref(1)',
  'product_variant_schema_ref(2)',
  'product_variant_translations',
  'ORDER BY v.index_revision',
  'index_entities',
  'index_links',
  'tokio::spawn',
  'tokio::time::sleep',
  'loop {',
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
requireMarkers(
  'crates/rustok-product/src/migrations/m20260731_000004_add_product_index_tombstones.rs',
  [
    'CREATE TABLE product_variant_index_tombstones',
    'rustok_product_variant_capture_index_tombstone',
    'trg_product_variants_capture_index_tombstone',
    'rustok_product_variant_seed_index_revision_from_tombstone',
    'rustok_product_variant_clear_superseded_index_tombstone',
  ],
);

requireMarkers('scripts/verify/verify-index-query-contract.mjs', [
  "'verify-index-product-variant-source.mjs'",
]);

console.log('[verify-index-product-variant-source] canonical ProductVariant source contract verified');

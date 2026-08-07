#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const read = (relative) => fs.readFileSync(path.join(root, relative), 'utf8');
const fail = (message) => {
  console.error(`[verify-index-product-tombstone-source] ${message}`);
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

const productPath = 'crates/rustok-distribution/src/product_index/product.rs';
const product = requireMarkers(productPath, [
  'PRODUCT_EVENT_DOMAIN: &str = "rustok-product.product-replay"',
  'FROM product_index_tombstones tombstone',
  'COUNT(*) OVER (',
  'identity_count != 1',
  'ProductRowState::Deleted',
  'IndexMutation::Delete {',
  'source_version: self.source_version',
  '(row.product_id, row.locale) > ($2, $3)',
  'ORDER BY row.product_id ASC, row.locale ASC',
  'WITH requested(product_id, locale) AS (VALUES {})',
]);
forbidMarkers(productPath, product, [
  'PRODUCT_EVENT_DOMAIN_V1',
  'PRODUCT_EVENT_DOMAIN_V2',
  'ProductSchemaVersion',
  'product_v1_schema',
  'product_v2_schema',
]);

const variantPath = 'crates/rustok-distribution/src/product_variant_index.rs';
const variant = requireMarkers(variantPath, [
  'PRODUCT_VARIANT_EVENT_DOMAIN: &str = "rustok-product.product-variant-replay"',
  'FROM product_variant_index_tombstones tombstone',
  'COUNT(*) OVER (',
  'identity_count != 1',
  'ProductVariantRowState::Deleted',
  'IndexMutation::Delete {',
  'source_version: self.source_version',
  'row.variant_id > $2',
  'ORDER BY row.variant_id ASC',
  'WITH requested(variant_id) AS (VALUES {})',
]);
forbidMarkers(variantPath, variant, [
  'PRODUCT_VARIANT_EVENT_DOMAIN_V1',
  'PRODUCT_VARIANT_EVENT_DOMAIN_V2',
  'ProductVariantSchemaVersion',
  'product_variant_v1_schema',
  'product_variant_v2_schema',
]);

const migrationPath =
  'crates/rustok-product/src/migrations/m20260731_000004_add_product_index_tombstones.rs';
const migration = requireMarkers(migrationPath, [
  'CREATE TABLE product_index_tombstones (',
  'PRIMARY KEY (tenant_id, product_id, locale)',
  'CREATE TABLE product_variant_index_tombstones (',
  'PRIMARY KEY (tenant_id, variant_id)',
  'rustok_product_store_index_tombstone(',
  'rustok_product_variant_store_index_tombstone(',
  'GREATEST(',
  'tombstone.source_version >= live_source_version',
  'source_version < live_source_version',
  'rustok_product_seed_index_revision_from_tombstones',
  'retained_source_version + 1',
  'trg_products_capture_index_tombstones',
  'BEFORE DELETE ON products',
  'OLD.index_revision + 1',
  'rustok_product_variant_seed_index_revision_from_tombstone',
  'trg_product_variants_capture_index_tombstone',
  'BEFORE DELETE ON product_variants',
]);
forbidMarkers(migrationPath, migration, [
  'index_entities',
  'index_links',
  'index_jobs',
  'index_checkpoints',
  'rustok_index',
]);

const productCargo = read('crates/rustok-product/Cargo.toml');
forbidMarkers('crates/rustok-product/Cargo.toml', productCargo, ['rustok-index']);

requireMarkers('scripts/verify/verify-index-query-contract.mjs', [
  "'verify-index-product-tombstone-source.mjs'",
]);

console.log('[verify-index-product-tombstone-source] canonical retained delete contract verified');

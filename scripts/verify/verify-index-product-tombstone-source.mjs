#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const resolve = (relative) => path.join(root, relative);
const read = (relative) => fs.readFileSync(resolve(relative), 'utf8');
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

const sourcePath = 'crates/rustok-distribution/src/product_index/graph.rs';
const source = requireMarkers(sourcePath, [
  'PRODUCT_INDEX_SOURCE: &str = "product-postgres-primary"',
  'PRODUCT_VARIANT_INDEX_SOURCE: &str = "product-variant-postgres-primary"',
  'PRODUCT_EVENT_DOMAIN_V1: &str = "rustok-product.product-replay-v1"',
  'PRODUCT_EVENT_DOMAIN_V2: &str = "rustok-product.product-replay-v2"',
  'PRODUCT_VARIANT_EVENT_DOMAIN_V1: &str = "rustok-product.product-variant-replay-v1"',
  'PRODUCT_VARIANT_EVENT_DOMAIN_V2: &str = "rustok-product.product-variant-replay-v2"',
  'FROM product_index_tombstones tombstone',
  'FROM product_variant_index_tombstones tombstone',
  'COUNT(*) OVER (',
  'AS identity_count',
  'identity_count != 1',
  'ProductRowState::Deleted',
  'ProductVariantRowState::Deleted',
  'IndexMutation::Delete {',
  'source_version: self.source_version',
  '(row.product_id, row.locale) > ($2, $3)',
  'ORDER BY row.product_id ASC, row.locale ASC',
  'row.variant_id > $2',
  'ORDER BY row.variant_id ASC',
  'WITH requested(product_id, locale) AS (VALUES {})',
  'WITH requested(variant_id) AS (VALUES {})',
  'retained_rows_emit_versioned_delete_mutations',
  'tombstone_sql_fails_closed_on_live_identity_coexistence',
]);
forbidMarkers(sourcePath, source, [
  'INSERT INTO index_',
  'UPDATE index_',
  'DELETE FROM index_',
  'tokio::spawn',
  'tokio::time::sleep',
  'loop {',
  'FROM channels',
  'JOIN channels',
  'rustok_channel',
  'rustok_search',
]);

const migrationPath =
  'crates/rustok-product/src/migrations/m20260731_000004_add_product_index_tombstones.rs';
const migration = requireMarkers(migrationPath, [
  'CREATE TABLE product_index_tombstones (',
  'PRIMARY KEY (tenant_id, product_id, locale)',
  'CREATE TABLE product_variant_index_tombstones (',
  'PRIMARY KEY (tenant_id, variant_id)',
  'chk_product_index_tombstones_source_version_positive',
  'chk_product_variant_index_tombstones_source_version_positive',
  'rustok_product_store_index_tombstone(',
  'rustok_product_variant_store_index_tombstone(',
  'ON CONFLICT (tenant_id, product_id, locale) DO UPDATE',
  'ON CONFLICT (tenant_id, variant_id) DO UPDATE',
  'GREATEST(',
  'tombstone.source_version >= live_source_version',
  'source_version < live_source_version',
  'rustok_product_seed_index_revision_from_tombstones',
  'retained_source_version + 1',
  'trg_products_capture_index_tombstones',
  'BEFORE DELETE ON products',
  'OLD.index_revision + 1',
  'OLD.locale IS DISTINCT FROM NEW.locale',
  'rustok_product_variant_seed_index_revision_from_tombstone',
  'trg_product_variants_capture_index_tombstone',
  'BEFORE DELETE ON product_variants',
  'trg_product_variants_clear_index_tombstone',
  'trg_product_variants_move_index_tombstone',
  'AFTER UPDATE OF id, tenant_id ON product_variants',
  'DROP TABLE IF EXISTS product_variant_index_tombstones;',
  'DROP TABLE IF EXISTS product_index_tombstones;',
]);
forbidMarkers(migrationPath, migration, [
  'index_entities',
  'index_links',
  'index_jobs',
  'index_checkpoints',
  'rustok_index',
]);

requireMarkers('crates/rustok-product/src/migrations/mod.rs', [
  'mod m20260731_000004_add_product_index_tombstones;',
  'Box::new(m20260731_000004_add_product_index_tombstones::Migration)',
]);

const productCargo = read('crates/rustok-product/Cargo.toml');
forbidMarkers('crates/rustok-product/Cargo.toml', productCargo, ['rustok-index']);

requireMarkers('crates/rustok-index/docs/m7-product-tombstone-source.md', [
  'Status: `source_complete_owner_execution_pending`',
  '`product_index_tombstones`',
  '`product_variant_index_tombstones`',
  '`IndexMutation::Delete`',
  '`identity_count`',
  'strictly greater than the last live row',
  'tombstone retention and purge policy',
  'maintainer-run',
]);
requireMarkers('crates/rustok-index/docs/m7-product-graph-source.md', [
  '[Product tombstone replay contract](m7-product-tombstone-source.md)',
  'retained hard-delete identities',
]);
requireMarkers('crates/rustok-product/README.md', [
  '`product_index_tombstones`',
  '`product_variant_index_tombstones`',
  '`IndexMutation::Delete`',
]);
requireMarkers('scripts/verify/verify-index-query-contract.mjs', [
  "'verify-index-product-tombstone-source.mjs'",
]);

console.log('[verify-index-product-tombstone-source] OK');

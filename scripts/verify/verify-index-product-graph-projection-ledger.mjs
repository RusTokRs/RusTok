#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const read = (relative) => fs.readFileSync(path.join(root, relative), 'utf8');
const fail = (message) => {
  console.error(`[verify-index-product-graph-projection-ledger] ${message}`);
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

const canonicalMigrationPath =
  'crates/rustok-product/src/migrations/m20260807_000010_canonicalize_product_index_graph_projection.rs';
const migration = requireMarkers(canonicalMigrationPath, [
  'RENAME TO product_index_graph_projection_snapshots',
  'rustok_product_guard_index_graph_projection_snapshot',
  'first Product Index graph projection epoch must equal 1',
  'Product Index graph projection epoch must advance exactly once',
  'Product Index graph projection input watermark regressed',
  'unchanged Product Index graph projection input must not append a new epoch',
  'rustok_product_reject_index_graph_projection_mutation',
  'product-index-graph-projection',
  'rustok_product_reconcile_index_graph_projection',
  'effective_product_source_version := GREATEST(',
  'effective_relation_epoch := GREATEST(',
  'previous_projection_epoch + 1',
  'FROM products product',
  'FROM product_index_tombstones tombstone',
  'FROM product_sales_channel_index_relation_snapshots relation',
  'trg_products_index_graph_projection_insert',
  'AFTER UPDATE OF index_revision ON products',
  'trg_products_zz_index_graph_projection_delete',
  'trg_product_channel_relation_index_graph_projection_insert',
]);
forbidMarkers(canonicalMigrationPath, migration, [
  'FROM channels',
  'JOIN channels',
  'index_entities',
  'index_links',
  'IndexMutation',
  'projection_epoch := GREATEST',
]);

const relationMigration = requireMarkers(
  'crates/rustok-product/src/migrations/m20260807_000008_add_product_sales_channel_index_relation_snapshots.rs',
  [
    'CREATE TRIGGER trg_products_retain_empty_channel_relation',
    'AFTER DELETE ON products',
    'CREATE TRIGGER trg_product_channel_relation_snapshot_insert',
  ],
);
if (!(migration.includes('trg_products_zz_index_graph_projection_delete') &&
      relationMigration.includes('trg_products_retain_empty_channel_relation'))) {
  fail('Product hard-delete projection ordering markers are incomplete');
}

requireMarkers('crates/rustok-product/src/migrations/mod.rs', [
  'mod m20260807_000010_canonicalize_product_index_graph_projection;',
  'Box::new(m20260807_000010_canonicalize_product_index_graph_projection::Migration)',
]);

const productSource = requireMarkers('crates/rustok-distribution/src/product_index/product.rs', [
  'product_index_graph_projection_snapshots',
  'projection.projection_epoch AS source_version',
  'projection.product_source_version AS projected_product_source_version',
  'projection.channel_ids AS sales_channel_ids',
  'name: link_name("sales_channels")?',
]);
forbidMarkers('crates/rustok-distribution/src/product_index/product.rs', productSource, [
  'product_index_graph_v3_projection_snapshots',
  'ProductSchemaVersion',
  'PRODUCT_EVENT_DOMAIN_V1',
  'PRODUCT_EVENT_DOMAIN_V2',
]);

const absence = requireMarkers('crates/rustok-distribution/src/product_index/absence.rs', [
  'product_index_graph_projection_snapshots',
  'projection.product_source_version = product.index_revision',
  'CAST(projection.projection_epoch AS TEXT) AS source_version_text',
]);
forbidMarkers('crates/rustok-distribution/src/product_index/absence.rs', absence, [
  'product_index_graph_v3_projection_snapshots',
  'CAST(product.index_revision AS TEXT)',
]);

requireMarkers('scripts/verify/verify-index-query-contract.mjs', [
  "'verify-index-product-graph-projection-ledger.mjs'",
]);

console.log('[verify-index-product-graph-projection-ledger] canonical projection contract verified');

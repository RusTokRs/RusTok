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

const productCargo = read('crates/rustok-product/Cargo.toml');
forbidMarkers('crates/rustok-product/Cargo.toml', productCargo, ['rustok-index', 'rustok-channel']);

const modulePath = 'crates/rustok-distribution/src/product_index/mod.rs';
const moduleSource = requireMarkers(modulePath, [
  'mod channel_visibility;',
  'mod product;',
  'mod variant;',
  'product::register(extensions)?;',
  'variant::register(extensions)?;',
  'absence::register(extensions)',
  'selected_product_bridge_registers_two_current_schemas_and_three_factories',
]);
forbidMarkers(modulePath, moduleSource, ['mod graph;', 'graph::', 'four_schemas']);

for (const removed of [
  'crates/rustok-distribution/src/product_index/graph.rs',
  'crates/rustok-index/docs/m7-product-source.md',
  'crates/rustok-index/docs/m7-product-variant-source.md',
  'crates/rustok-product/docs/index-graph-v3-projection-ledger.md',
  'scripts/verify/verify-index-product-graph-source.mjs',
  'scripts/verify/verify-index-product-v3-projection-ledger.mjs',
]) {
  if (fs.existsSync(resolve(removed))) {
    fail(`removed Product compatibility artifact still exists: ${removed}`);
  }
}

const sourcePath = 'crates/rustok-distribution/src/product_index/product.rs';
const source = requireMarkers(sourcePath, [
  'PRODUCT_INDEX_SOURCE: &str = "product-postgres-primary"',
  'PRODUCT_EVENT_DOMAIN: &str = "rustok-product.product-replay"',
  'fn product_schema()',
  'locale_mode: LocaleMode::Required',
  'many_field("variant_ids", IndexValueType::Uuid, true)?',
  'many_field("sales_channel_ids", IndexValueType::Uuid, true)?',
  'name: link_name("variants")?',
  'name: link_name("sales_channels")?',
  'target_schema: product_variant_schema_ref()?',
  'target_schema: sales_channel_schema_ref()?',
  'assert_eq!(schema.fields.len(), 10);',
  'assert_eq!(schema.links.len(), 2);',
  'product_index_graph_projection_snapshots',
  'product_sales_channel_index_relation_snapshots',
  'product_sales_channel_index_relation_freshness_snapshots',
  'channel_index_identity_generations',
  'projection.projection_epoch AS source_version',
  'projection.channel_ids AS sales_channel_ids',
  'freshness.visibility_key AS freshness_visibility_key',
  'freshness.channel_identity_generation AS freshness_channel_identity_generation',
  'decode_product_visibility(&metadata)',
  'freshness_visibility_key != current_visibility_key',
  'freshness_channel_identity_generation != current_channel_identity_generation',
  'freshness_product_source_version > observed_product_source_version',
  'projected_product_source_version != observed_product_source_version',
  'does not require a live freshness witness',
  'FROM products p',
  'JOIN product_translations t',
  'FROM product_index_tombstones tombstone',
  'jsonb_agg(v.id ORDER BY v.id)',
  '(row.product_id, row.locale) > ($2, $3)',
  'ORDER BY row.product_id ASC, row.locale ASC',
  'WITH requested(product_id, locale) AS (VALUES {})',
  'IndexMutation::Delete {',
  'IndexMutation::Upsert {',
  'canonical_product_schema_contains_only_current_fields_and_links',
  'canonical_product_registration_publishes_one_schema_and_one_source_factory',
  'canonical_product_projection_sql_requires_owner_projection_relation_and_freshness',
]);
forbidMarkers(sourcePath, source, [
  'ProductSchemaVersion',
  'product_v1_schema',
  'product_v2_schema',
  'PRODUCT_EVENT_DOMAIN_V1',
  'PRODUCT_EVENT_DOMAIN_V2',
  'product-replay-v1',
  'product-replay-v2',
  'FROM channels',
  'JOIN channels',
  'index_entities',
  'index_links',
  'tokio::spawn',
  'tokio::time::sleep',
  'loop {',
]);

const absencePath = 'crates/rustok-distribution/src/product_index/absence.rs';
const absence = requireMarkers(absencePath, [
  'PRODUCT_ABSENCE_WATERMARK_FACTORY',
  'product-locale-absence-postgres',
  '[product_schema_ref()?]',
  'product_index_graph_projection_snapshots',
  'projection.product_source_version = product.index_revision',
  'product_sales_channel_index_relation_snapshots',
  'product_sales_channel_index_relation_freshness_snapshots',
  'channel_index_identity_generations',
  'decode_product_visibility(&metadata)',
  'freshness_visibility_key != current_visibility_key',
  'freshness_channel_identity_generation != current_channel_identity_generation',
  'FROM product_translations translation',
  'FROM product_index_tombstones tombstone',
  'IndexSourceAbsenceWatermark::new(key, source_version)',
]);
forbidMarkers(absencePath, absence, [
  'product_schema_ref(1)',
  'product_schema_ref(2)',
  'product_index_graph_v3_projection_snapshots',
  'CAST(product.index_revision AS TEXT)',
  'INSERT ',
  'UPDATE ',
  'DELETE FROM',
  'index_entities',
  'index_links',
  'tokio::spawn',
  'loop {',
]);

const canonicalMigration = requireMarkers(
  'crates/rustok-product/src/migrations/m20260807_000010_canonicalize_product_index_graph_projection.rs',
  [
    'product_index_graph_projection_snapshots',
    'rustok_product_guard_index_graph_projection_snapshot',
    'rustok_product_reconcile_index_graph_projection',
    'trg_products_zz_index_graph_projection_delete',
    'trg_product_channel_relation_index_graph_projection_insert',
  ],
);
forbidMarkers(
  'crates/rustok-product/src/migrations/m20260807_000010_canonicalize_product_index_graph_projection.rs',
  canonicalMigration,
  ['FROM channels', 'JOIN channels', 'index_entities', 'index_links', 'IndexMutation'],
);

requireMarkers('scripts/verify/verify-index-query-contract.mjs', [
  "'verify-index-product-source.mjs'",
  "'verify-index-product-channel-relation-freshness.mjs'",
]);

console.log('[verify-index-product-source] canonical Product source and freshness gate verified');

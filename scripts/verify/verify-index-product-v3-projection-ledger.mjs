#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const read = (relative) => fs.readFileSync(path.join(root, relative), 'utf8');
const fail = (message) => {
  console.error(`[verify-index-product-v3-projection-ledger] ${message}`);
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

const migrationPath =
  'crates/rustok-product/src/migrations/m20260807_000009_add_product_index_graph_v3_projection_snapshots.rs';
const migration = requireMarkers(migrationPath, [
  'CREATE TABLE product_index_graph_v3_projection_snapshots',
  'projection_epoch BIGINT NOT NULL',
  'product_source_version BIGINT NOT NULL',
  'relation_epoch BIGINT NOT NULL',
  'UNIQUE (tenant_id, product_id, product_source_version, relation_epoch)',
  'rustok_product_guard_index_graph_v3_projection_snapshot',
  'first Product Index graph v3 projection epoch must equal 1',
  'projection epoch must advance exactly once',
  'projection input watermark regressed',
  'unchanged Product Index graph v3 projection input must not append a new epoch',
  'rustok_product_reject_index_graph_v3_projection_mutation',
  'product-index-graph-v3-projection',
  'rustok_product_reconcile_index_graph_v3_projection',
  'effective_product_source_version := GREATEST(',
  'effective_relation_epoch := GREATEST(',
  'previous_projection_epoch + 1',
  'FROM products product',
  'FROM product_index_tombstones tombstone',
  'FROM product_sales_channel_index_relation_snapshots relation',
  'WITH live_product_versions AS (',
  'retained_product_versions AS (',
  'current_relations AS (',
  'trg_products_index_graph_v3_projection_insert',
  'AFTER UPDATE OF index_revision ON products',
  'trg_products_zz_index_graph_v3_projection_delete',
  'trg_product_channel_relation_index_graph_v3_projection_insert',
]);
forbidMarkers(migrationPath, migration, [
  'FROM channels',
  'JOIN channels',
  'index_entities',
  'index_links',
  'IndexMutation',
  'tokio::spawn',
]);

requireMarkers('crates/rustok-product/src/migrations/mod.rs', [
  'mod m20260807_000009_add_product_index_graph_v3_projection_snapshots;',
  'Box::new(m20260807_000009_add_product_index_graph_v3_projection_snapshots::Migration)',
]);

requireMarkers('crates/rustok-product/docs/index-graph-v3-projection-ledger.md', [
  'Status: `projection_epoch_source_complete_v3_replay_pending`',
  'Index mutation store accepts only full `Upsert`/`Delete` mutations',
  '`projection_epoch` remains the sole future v3 source version',
  'does **not** prove that the relation membership is fresh',
  'Product v3 replay must therefore remain non-authoritative',
]);
requireMarkers('crates/rustok-index/docs/m7-product-sales-channel-relation-admission.md', [
  'resolver_and_projection_epoch_source_complete_v3_wiring_and_runtime_evidence_pending',
  'Product v3 projection epoch',
  'full Product v3 record cannot safely use either',
  'The separate `projection_epoch` is the future Product v3 full-record source version',
]);
requireMarkers('crates/rustok-index/docs/implementation-plan-current-2026-08-07.md', [
  'Product-owned graph-v3 projection epoch ledger',
  'Using either counter directly would permit a change from the other family to be stale-ignored',
  'Remaining freshness boundary',
  'publish Product v3 on the existing stable',
]);

const productGraph = read('crates/rustok-distribution/src/product_index/graph.rs');
forbidMarkers('crates/rustok-distribution/src/product_index/graph.rs', productGraph, [
  'PRODUCT_EVENT_DOMAIN_V3',
  'ProductSchemaVersion::V3',
  'product_schema_ref(3)',
  'sales_channels',
  'sales_channel_ids',
]);
const productAbsence = read('crates/rustok-distribution/src/product_index/absence.rs');
forbidMarkers('crates/rustok-distribution/src/product_index/absence.rs', productAbsence, [
  'product_schema_ref(3)',
  'product_index_graph_v3_projection_snapshots',
]);
const deferredSourcePath = path.join(
  root,
  'crates/rustok-distribution/src/product_index/graph_v3.rs',
);
if (fs.existsSync(deferredSourcePath)) {
  fail('Product v3 replay source must remain deferred in the projection-epoch prerequisite slice');
}

const aggregate = read('scripts/verify/verify-index-query-contract.mjs');
if (!aggregate.includes("'verify-index-product-v3-projection-ledger.mjs'")) {
  fail('Index aggregate verifier does not include the Product v3 projection ledger guard');
}

console.log('[verify-index-product-v3-projection-ledger] Product v3 projection epoch contract verified');

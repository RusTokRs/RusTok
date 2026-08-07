#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const read = (relative) => fs.readFileSync(path.join(root, relative), 'utf8');
const fail = (message) => {
  console.error(`[verify-index-product-channel-relation-ledger] ${message}`);
  process.exit(1);
};
const requireMarkers = (relative, markers) => {
  const source = read(relative);
  for (const marker of markers) {
    if (!source.includes(marker)) fail(`${relative} is missing ${marker}`);
  }
  return source;
};

const migrationPath =
  'crates/rustok-product/src/migrations/m20260807_000008_add_product_sales_channel_index_relation_snapshots.rs';
const migration = requireMarkers(migrationPath, [
  'CREATE TABLE product_sales_channel_index_relation_snapshots',
  'relation_epoch BIGINT NOT NULL',
  'channel_ids JSONB NOT NULL',
  'UNIQUE (tenant_id, sequence_no)',
  'rustok_product_validate_channel_relation_ids',
  'jsonb_array_length(value) > 1024',
  'rustok_product_guard_channel_relation_snapshot',
  'pg_advisory_xact_lock(hashtextextended(lock_key, 0))',
  'first Product-SalesChannel relation epoch must equal 1',
  'relation epoch must advance exactly once',
  'unchanged Product-SalesChannel membership must not append a new epoch',
  'Product-SalesChannel relation snapshots are append-only',
  'AFTER DELETE ON products',
  "previous_channel_ids <> '[]'::jsonb",
]);
for (const forbidden of [
  'REFERENCES channels',
  'FROM channels',
  'JOIN channels',
  'index_entities',
  'index_links',
]) {
  if (migration.includes(forbidden)) {
    fail(`${migrationPath} contains forbidden cross-owner or Index storage coupling: ${forbidden}`);
  }
}

const servicePath = 'crates/rustok-product/src/services/index_channel_relation.rs';
const service = requireMarkers(servicePath, [
  'pub const MAX_PRODUCT_SALES_CHANNEL_RELATION_CHANNELS: usize = 1024;',
  'pub const MAX_PRODUCT_SALES_CHANNEL_RELATION_PAGE: usize = 256;',
  'pub const MAX_PRODUCT_SALES_CHANNEL_RELATION_TARGETS: usize = 64;',
  'pub struct ProductSalesChannelIndexRelationRecord',
  'pub enum ProductSalesChannelIndexRelationWriteOutcome',
  'pub struct ProductSalesChannelIndexRelationStore',
  'pub async fn replace(',
  'pub async fn list_changes(',
  'pub async fn scan_current(',
  'pub async fn load_current(',
  'ProductSalesChannelIndexRelationWriteOutcome::Unchanged',
  'ProductSalesChannelIndexRelationError::ProductNotFound',
  'require_live_product(transaction, tenant_id, product_id).await?',
  'FOR KEY SHARE',
  'checked_add(1)',
  'INSERT INTO product_sales_channel_index_relation_snapshots',
  'SELECT pg_advisory_xact_lock(hashtextextended($1, 0))',
  'ORDER BY product_id ASC, relation_epoch DESC',
  'ORDER BY sequence_no ASC',
]);
for (const forbidden of [
  'rustok_channel',
  'rustok_index',
  'IndexMutation',
  'FROM channels',
  'JOIN channels',
  'tokio::spawn',
  'loop {',
]) {
  if (service.includes(forbidden)) {
    fail(`${servicePath} contains forbidden resolver, Index, or runtime coupling: ${forbidden}`);
  }
}

const cargo = read('crates/rustok-product/Cargo.toml');
for (const forbidden of ['rustok-channel', 'rustok-index']) {
  if (cargo.includes(forbidden)) {
    fail(`rustok-product must not gain a ${forbidden} dependency for relation ownership`);
  }
}

requireMarkers('crates/rustok-product/src/migrations/mod.rs', [
  'mod m20260807_000008_add_product_sales_channel_index_relation_snapshots;',
  'Box::new(m20260807_000008_add_product_sales_channel_index_relation_snapshots::Migration)',
]);
requireMarkers('crates/rustok-product/src/services/mod.rs', [
  'mod index_channel_relation;',
  'ProductSalesChannelIndexRelationStore',
  'ProductSalesChannelIndexRelationWriteOutcome',
]);
requireMarkers('crates/rustok-product/src/lib.rs', [
  'ProductSalesChannelIndexRelationRecord',
  'ProductSalesChannelIndexRelationStore',
  'MAX_PRODUCT_SALES_CHANNEL_RELATION_CHANNELS',
]);

const ledgerDoc = requireMarkers('crates/rustok-product/docs/index-sales-channel-relation-ledger.md', [
  'Status: `canonical_graph_source_complete_freshness_and_runtime_evidence_pending`',
  '`product_sales_channel_index_relation_snapshots`',
  '`ProductSalesChannelIndexRelationStore::replace`',
  '`FOR KEY SHARE`',
  '`product_index_graph_projection_snapshots.projection_epoch`',
  '`sales_channel_ids`',
  '`sales_channels` link',
  'no `rustok-index` or `rustok-channel` dependency',
  'No tests, Node verifiers, Cargo checks',
]);
for (const legacy of ['Product v1', 'Product v2', 'Product v3', 'new Product schema version']) {
  if (ledgerDoc.includes(legacy)) fail(`relation ledger doc retains legacy compatibility text: ${legacy}`);
}

requireMarkers('crates/rustok-index/docs/m7-product-sales-channel-relation-admission.md', [
  'current Product Index graph already contains the Product-to-SalesChannel link',
  '`product_index_graph_projection_snapshots`',
  'Durable Product/Channel convergence triggering',
]);
requireMarkers('crates/rustok-index/docs/implementation-plan-current-2026-08-07.md', [
  'Product-owned Product-to-SalesChannel relation snapshots',
  'one canonical Product Index source',
]);

const aggregate = read('scripts/verify/verify-index-query-contract.mjs');
if (!aggregate.includes("'verify-index-product-channel-relation-ledger.mjs'")) {
  fail('Index aggregate verifier does not include the Product-SalesChannel relation ledger guard');
}

console.log('[verify-index-product-channel-relation-ledger] canonical Product-SalesChannel owner ledger verified');

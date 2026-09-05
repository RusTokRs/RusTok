#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const read = (relative) => fs.readFileSync(path.join(root, relative), 'utf8');
const fail = (message) => {
  console.error(`[verify-index-product-channel-relation-freshness] ${message}`);
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

const channelMigrationPath =
  'crates/rustok-channel/src/migrations/m20260807_000012_add_channel_index_identity_generation.rs';
const channelMigration = requireMarkers(channelMigrationPath, [
  'CREATE TABLE channel_index_identity_generations',
  'tenant_id UUID PRIMARY KEY',
  'generation BIGINT NOT NULL',
  'rustok_channel_bump_index_identity_generation',
  'channel-index-identity-generation',
  'rustok_channel_track_index_identity_generation',
  'AFTER INSERT OR DELETE OR UPDATE OF id, tenant_id, slug ON channels',
  'lower(btrim(OLD.slug)) IS NOT DISTINCT FROM lower(btrim(NEW.slug))',
  'old_tenant::text < new_tenant::text',
  'generation = previous_generation + 1',
  'FOR seed_tenant_id IN',
  'ORDER BY seeded.tenant_id::text',
  'PERFORM rustok_channel_bump_index_identity_generation(seed_tenant_id)',
]);
forbidMarkers(channelMigrationPath, channelMigration, [
  'is_active',
  'channel_targets',
  'channel_oauth_apps',
  'channel_resolution_policy',
  'index_entities',
  'index_links',
]);
requireMarkers('crates/rustok-channel/src/migrations/mod.rs', [
  'mod m20260807_000012_add_channel_index_identity_generation;',
  'Box::new(m20260807_000012_add_channel_index_identity_generation::Migration)',
]);

const freshnessMigrationPath =
  'crates/rustok-product/src/migrations/m20260807_000011_add_product_sales_channel_relation_freshness.rs';
const freshnessMigration = requireMarkers(freshnessMigrationPath, [
  'CREATE TABLE product_sales_channel_index_relation_freshness_snapshots',
  'relation_epoch BIGINT NOT NULL',
  'product_source_version BIGINT NOT NULL',
  'visibility_key TEXT NOT NULL',
  'channel_identity_generation BIGINT NOT NULL',
  'FOREIGN KEY (tenant_id, product_id, relation_epoch)',
  'REFERENCES product_sales_channel_index_relation_snapshots',
  'octet_length(visibility_key) BETWEEN 1 AND 131072',
  'rustok_product_guard_channel_relation_freshness_snapshot',
  'FOR KEY SHARE',
  'freshness witness requires a live Product',
  'product-sales-channel-index-relation',
  'product-sales-channel-index-relation-freshness',
  'freshness witness requires the current relation epoch',
  'freshness relation epoch regressed',
  'freshness Product watermark regressed',
  'freshness Channel watermark regressed',
  'unchanged Product-SalesChannel freshness witness must not append',
  'relation freshness snapshots are append-only',
]);
forbidMarkers(freshnessMigrationPath, freshnessMigration, [
  'FROM channels',
  'JOIN channels',
  'REFERENCES channels',
  'index_entities',
  'index_links',
]);
requireMarkers('crates/rustok-product/src/migrations/mod.rs', [
  'mod m20260807_000011_add_product_sales_channel_relation_freshness;',
  'Box::new(m20260807_000011_add_product_sales_channel_relation_freshness::Migration)',
]);

const freshnessStorePath =
  'crates/rustok-product/src/services/index_channel_relation_freshness.rs';
const freshnessStore = requireMarkers(freshnessStorePath, [
  'MAX_PRODUCT_SALES_CHANNEL_VISIBILITY_KEY_BYTES: usize = 131_072',
  'RELATION_LOCK_DOMAIN: &str = "product-sales-channel-index-relation"',
  'FRESHNESS_LOCK_DOMAIN: &str = "product-sales-channel-index-relation-freshness"',
  'pub struct ProductSalesChannelIndexRelationFreshnessRecord',
  'pub struct ProductSalesChannelIndexRelationFreshnessStore',
  'pub async fn record(',
  'require_live_product(transaction, tenant_id, product_id).await?',
  'FOR KEY SHARE',
  'RELATION_LOCK_DOMAIN,',
  'FRESHNESS_LOCK_DOMAIN,',
  'require_current_relation_epoch(transaction, tenant_id, product_id, relation_epoch).await?',
  'ProductSalesChannelIndexRelationFreshnessError::RelationNotCurrent',
  'ProductSalesChannelIndexRelationFreshnessError::WatermarkRegressed',
  'INSERT INTO product_sales_channel_index_relation_freshness_snapshots',
  'ORDER BY sequence_no DESC',
]);
forbidMarkers(freshnessStorePath, freshnessStore, [
  'rustok_channel',
  'rustok_index',
  'FROM channels',
  'JOIN channels',
  'IndexMutation',
  'tokio::spawn',
  'loop {',
]);
const productCargo = read('crates/rustok-product/Cargo.toml');
forbidMarkers('crates/rustok-product/Cargo.toml', productCargo, ['rustok-channel', 'rustok-index']);

const visibilityPath = 'crates/rustok-distribution/src/product_index/channel_visibility.rs';
const visibility = requireMarkers(visibilityPath, [
  'MAX_PRODUCT_SALES_CHANNEL_VISIBILITY_SLUGS: usize = 1024',
  'MAX_PRODUCT_SALES_CHANNEL_VISIBILITY_SLUG_BYTES: usize = 100',
  'ProductChannelVisibility::Unrestricted',
  'ProductChannelVisibility::Restricted',
  'pub(crate) fn freshness_key(&self) -> String',
  '"all".to_owned()',
  '"restricted:{}"',
  'decode_product_visibility(',
]);
forbidMarkers(visibilityPath, visibility, ['IndexMutation', 'FROM channels', 'JOIN channels']);

const resolverPath = 'crates/rustok-distribution/src/product_index/channel_relation_resolver.rs';
const resolver = requireMarkers(resolverPath, [
  'ProductSalesChannelIndexRelationFreshnessStore::new',
  'load_channel_identity_generation(&transaction, tenant_id).await?',
  'channel_index_identity_generations',
  'verified.channel_ids == outcome.record().channel_ids()',
  '.record(',
  'verified.product_source_version',
  'verified.visibility_key.clone()',
  'verified.channel_identity_generation',
  'MAX_PRODUCT_SALES_CHANNEL_STABILIZATION_ATTEMPTS: usize = 3',
]);
forbidMarkers(resolverPath, resolver, [
  'IndexMutation',
  'index_entities',
  'index_links',
  'tokio::spawn',
  'sys_events',
  'OutboxRelay',
]);

const productSourcePath = 'crates/rustok-distribution/src/product_index/product.rs';
requireMarkers(productSourcePath, [
  'product_sales_channel_index_relation_freshness_snapshots',
  'channel_index_identity_generations',
  'freshness.visibility_key AS freshness_visibility_key',
  'freshness.channel_identity_generation AS freshness_channel_identity_generation',
  'decode_product_visibility(&metadata)',
  'freshness_visibility_key != current_visibility_key',
  'freshness_channel_identity_generation != current_channel_identity_generation',
  'freshness_product_source_version > observed_product_source_version',
  'does not require a live freshness witness',
]);

const absencePath = 'crates/rustok-distribution/src/product_index/absence.rs';
const absence = requireMarkers(absencePath, [
  'product_sales_channel_index_relation_freshness_snapshots',
  'channel_index_identity_generations',
  'decode_product_visibility(&metadata)',
  'freshness_visibility_key != current_visibility_key',
  'freshness_channel_identity_generation != current_channel_identity_generation',
  'return Ok(None);',
]);
forbidMarkers(absencePath, absence, ['INSERT ', 'UPDATE ', 'DELETE FROM']);

requireMarkers('crates/rustok-product/docs/index-sales-channel-relation-freshness.md', [
  'Status: `source_convergence_and_materialized_fence_complete_runtime_evidence_pending`',
  'freshness-only change does not pretend that the graph membership changed',
  '`channel_index_identity_generations`',
  'fails closed at source observation',
  '## Automatic convergence',
  'generic ModuleWork scheduler',
  '## Materialized freshness boundary',
  'canonical Index query boundary now supplies the separate',
  'cannot become query-authoritative',
  'first retained PostgreSQL materialized-freshness packet is source-ready',
]);
requireMarkers('crates/rustok-index/docs/m7-product-sales-channel-resolver.md', [
  'Status: `automatic_convergence_and_query_fence_source_complete_runtime_evidence_pending`',
  'ProductSalesChannelIndexRelationFreshnessStore::record',
  'Automatic convergence composition',
  'Automatic convergence now re-establishes stale/missing relation freshness',
  'materialized/query freshness fence separately closes',
]);
requireMarkers('crates/rustok-index/docs/m7-product-sales-channel-convergence.md', [
  'Automatic relation convergence is now source complete',
  'materialized/query freshness fence is also source complete',
]);
requireMarkers('crates/rustok-index/docs/m7-product-graph-source.md', [
  'Status: `single_current_product_and_storefront_query_source_complete_execution_admission_pending`',
]);
requireMarkers('crates/rustok-index/docs/implementation-plan-current-2026-08-07.md', [
  'Product-SalesChannel freshness witness',
  'Channel identity generation',
  'Freshness watermark source complete',
  'Automatic owner-change relation convergence source complete',
  'Materialized/query freshness fence source complete',
  'source-read -> mutation-apply',
]);

const aggregate = read('scripts/verify/verify-index-query-contract.mjs');
for (const expected of [
  "'verify-index-product-channel-relation-freshness.mjs'",
  "'verify-index-product-channel-relation-convergence.mjs'",
  "'verify-index-product-materialized-query-freshness.mjs'",
]) {
  if (!aggregate.includes(expected)) {
    fail(`Index aggregate verifier is missing ${expected}`);
  }
}

console.log('[verify-index-product-channel-relation-freshness] source freshness, convergence, and materialized fence boundary verified');

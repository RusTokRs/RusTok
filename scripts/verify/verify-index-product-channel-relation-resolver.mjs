#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const read = (relative) => fs.readFileSync(path.join(root, relative), 'utf8');
const fail = (message) => {
  console.error(`[verify-index-product-channel-relation-resolver] ${message}`);
  process.exit(1);
};
const requireMarkers = (relative, markers) => {
  const source = read(relative);
  for (const marker of markers) {
    if (!source.includes(marker)) fail(`${relative} is missing ${marker}`);
  }
  return source;
};

const visibilityPath = 'crates/rustok-distribution/src/product_index/channel_visibility.rs';
requireMarkers(visibilityPath, [
  'MAX_PRODUCT_SALES_CHANNEL_VISIBILITY_SLUGS: usize = 1024',
  'MAX_PRODUCT_SALES_CHANNEL_VISIBILITY_SLUG_BYTES: usize = 100',
  'pub(crate) fn freshness_key(&self) -> String',
  'decode_product_visibility(',
]);

const resolverPath =
  'crates/rustok-distribution/src/product_index/channel_relation_resolver.rs';
const resolver = requireMarkers(resolverPath, [
  'MAX_PRODUCT_SALES_CHANNEL_RELATION_RESOLVE_PAGE: usize = 64',
  'MAX_PRODUCT_SALES_CHANNEL_STABILIZATION_ATTEMPTS: usize = 3',
  'pub(crate) struct ProductSalesChannelRelationResolver',
  'pub(crate) async fn reconcile_product(',
  'pub(crate) async fn reconcile_tenant_page(',
  'IsolationLevel::RepeatableRead',
  'AccessMode::ReadOnly',
  'ProductSalesChannelIndexRelationStore::new',
  'ProductSalesChannelIndexRelationFreshnessStore::new',
  '.replace(tenant_id, product_id',
  'load_channel_identity_generation(&transaction, tenant_id).await?',
  'channel_index_identity_generations',
  'lower(btrim(slug)) IN',
  'SELECT id FROM channels WHERE tenant_id = $1 ORDER BY id ASC LIMIT $2',
  'ProductChannelVisibility::Unrestricted',
  'verified.channel_ids == outcome.record().channel_ids()',
  'verified.product_source_version',
  'verified.visibility_key.clone()',
  'verified.channel_identity_generation',
  'ProductSalesChannelIndexRelationFreshnessError::WatermarkRegressed',
  'ProductSalesChannelRelationResolverError::TooManyResolvedChannels',
  'ProductSalesChannelRelationResolverError::ConcurrentChange',
  'ProductSalesChannelRelationResolverError::ProductNotFound',
  'assert!(!sql.contains("is_active"))',
]);
for (const forbidden of [
  'IndexMutation',
  'index_entities',
  'index_links',
  'tokio::spawn',
  'loop {',
  'sys_events',
  'TransactionalEventBus',
  'OutboxRelay',
]) {
  if (resolver.includes(forbidden)) {
    fail(`${resolverPath} contains forbidden Index/event/runtime coupling: ${forbidden}`);
  }
}

for (const ownerPath of [
  'crates/rustok-product/src/services/index_channel_relation.rs',
  'crates/rustok-product/src/services/index_channel_relation_freshness.rs',
]) {
  const owner = read(ownerPath);
  for (const forbidden of ['rustok_channel', 'rustok_index', 'FROM channels', 'JOIN channels']) {
    if (owner.includes(forbidden)) {
      fail(`${ownerPath} must remain Channel/Index independent: ${forbidden}`);
    }
  }
}

requireMarkers('crates/rustok-distribution/src/product_index/mod.rs', [
  'pub(crate) mod channel_relation_resolver;',
  'mod channel_visibility;',
]);

const resolverDoc = requireMarkers('crates/rustok-index/docs/m7-product-sales-channel-resolver.md', [
  'Status: `freshness_watermark_source_complete_runtime_evidence_pending`',
  '`REPEATABLE READ`, `READ ONLY`',
  '1024 visibility slugs',
  '64 Products per',
  'three stabilization attempts',
  '`channels.is_active` is not relation identity state',
  'ProductSalesChannelIndexRelationFreshnessStore::record',
  'source-level freshness watermark',
  'canonical Product Index source already materializes the `sales_channels` link',
  'No tests, Node verifiers, Cargo checks',
]);
for (const legacy of ['Product v1', 'Product v2', 'Product v3', 'new Product Index schema version']) {
  if (resolverDoc.includes(legacy)) fail(`resolver doc retains legacy compatibility text: ${legacy}`);
}

requireMarkers('crates/rustok-index/docs/m7-product-sales-channel-relation-admission.md', [
  'current Product Index graph contains the Product-to-SalesChannel link',
  'Product-owned freshness witness',
  'Channel identity generation',
]);
requireMarkers('crates/rustok-index/docs/implementation-plan-current-2026-08-07.md', [
  'bounded cross-owner Product visibility to SalesChannel UUID resolver',
  'one canonical Product Index source',
  'freshness watermark source complete',
]);

const aggregate = read('scripts/verify/verify-index-query-contract.mjs');
if (!aggregate.includes("'verify-index-product-channel-relation-resolver.mjs'")) {
  fail('Index aggregate verifier does not include the Product-SalesChannel resolver guard');
}

console.log('[verify-index-product-channel-relation-resolver] resolver and freshness witness verified');

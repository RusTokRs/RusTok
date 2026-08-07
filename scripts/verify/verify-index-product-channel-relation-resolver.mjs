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

const resolverPath =
  'crates/rustok-distribution/src/product_index/channel_relation_resolver.rs';
const resolver = requireMarkers(resolverPath, [
  'MAX_PRODUCT_SALES_CHANNEL_RELATION_RESOLVE_PAGE: usize = 64',
  'MAX_PRODUCT_SALES_CHANNEL_VISIBILITY_SLUGS: usize = 1024',
  'MAX_PRODUCT_SALES_CHANNEL_STABILIZATION_ATTEMPTS: usize = 3',
  'pub(crate) struct ProductSalesChannelRelationResolver',
  'pub(crate) async fn reconcile_product(',
  'pub(crate) async fn reconcile_tenant_page(',
  'IsolationLevel::RepeatableRead',
  'AccessMode::ReadOnly',
  'ProductSalesChannelIndexRelationStore::new',
  '.replace(tenant_id, product_id',
  'lower(btrim(slug)) IN',
  'SELECT id FROM channels WHERE tenant_id = $1 ORDER BY id ASC LIMIT $2',
  'ProductChannelVisibility::Unrestricted',
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

const ownerPath = 'crates/rustok-product/src/services/index_channel_relation.rs';
const owner = read(ownerPath);
for (const forbidden of ['rustok_channel', 'rustok_index', 'FROM channels', 'JOIN channels']) {
  if (owner.includes(forbidden)) {
    fail(`${ownerPath} must remain Channel/Index independent: ${forbidden}`);
  }
}

requireMarkers('crates/rustok-distribution/src/product_index/mod.rs', [
  'pub(crate) mod channel_relation_resolver;',
]);

const resolverDoc = requireMarkers('crates/rustok-index/docs/m7-product-sales-channel-resolver.md', [
  'Status: `source_complete_durable_triggering_and_runtime_evidence_pending`',
  '`REPEATABLE READ`, `READ ONLY`',
  'at most 64 Products',
  'at most three exact Product stabilization attempts',
  'does **not** filter `channels.is_active`',
  'not an atomic cross-owner snapshot',
  'canonical Product Index source already consumes the resulting relation',
  '`sales_channels` link',
  'No tests, Node verifiers, Cargo checks',
]);
for (const legacy of ['Product v1', 'Product v2', 'Product v3', 'new Product Index schema version']) {
  if (resolverDoc.includes(legacy)) fail(`resolver doc retains legacy compatibility text: ${legacy}`);
}

requireMarkers('crates/rustok-index/docs/m7-product-sales-channel-relation-admission.md', [
  'current Product Index graph already contains the Product-to-SalesChannel link',
  'bounded resolved membership discovery',
  'Durable Product/Channel convergence triggering',
]);
requireMarkers('crates/rustok-index/docs/implementation-plan-current-2026-08-07.md', [
  'bounded cross-owner Product visibility to SalesChannel UUID resolver',
  'one canonical Product Index source',
  'durable relation convergence/freshness triggering',
]);

const aggregate = read('scripts/verify/verify-index-query-contract.mjs');
if (!aggregate.includes("'verify-index-product-channel-relation-resolver.mjs'")) {
  fail('Index aggregate verifier does not include the Product-SalesChannel resolver guard');
}

console.log('[verify-index-product-channel-relation-resolver] canonical resolver contract verified');

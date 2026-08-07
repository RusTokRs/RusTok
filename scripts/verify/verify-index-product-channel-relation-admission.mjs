#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const read = (relative) => fs.readFileSync(path.join(root, relative), 'utf8');
const fail = (message) => {
  console.error(`[verify-index-product-channel-relation-admission] ${message}`);
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

requireMarkers('crates/rustok-distribution/src/product_index/mod.rs', [
  'pub(crate) mod relation_admission;',
]);

const admissionPath = 'crates/rustok-distribution/src/product_index/relation_admission.rs';
const admission = requireMarkers(admissionPath, [
  'PRODUCT_SALES_CHANNEL_RELATION_EVENT_DOMAIN',
  'rustok-distribution.product-sales-channel-relation',
  'ProductSalesChannelRelationEpoch',
  'ProductSalesChannelRelationSnapshot',
  'ProductSalesChannelRelationAdmission::Initial',
  'ProductSalesChannelRelationAdmission::Retry',
  'ProductSalesChannelRelationAdmission::Advanced',
  'SameEpochMembershipChanged',
  'EpochRegressed',
  'ScopeChanged',
  'channel_ids.sort_unstable();',
  'channel_ids.windows(2)',
  'derive_index_source_event_id(',
  'Some(&locale)',
  'epoch.get()',
]);
forbidMarkers(admissionPath, admission, [
  'EVENT_DOMAIN_V1',
  'relation-v1',
  'SystemTime',
  'Instant',
  'DefaultHasher',
  'product_revision.max',
  'channel_revision.max',
  'Utc::now',
  'CURRENT_TIMESTAMP',
]);

const productPath = 'crates/rustok-distribution/src/product_index/product.rs';
const product = requireMarkers(productPath, [
  'many_field("sales_channel_ids", IndexValueType::Uuid, true)?',
  'name: link_name("sales_channels")?',
  'target_schema: sales_channel_schema_ref()?',
  'projection.channel_ids AS sales_channel_ids',
  'product_index_graph_projection_snapshots',
  'product_sales_channel_index_relation_freshness_snapshots',
  'channel_index_identity_generations',
  'assert_eq!(schema.fields.len(), 10);',
  'assert_eq!(schema.links.len(), 2);',
]);
forbidMarkers(productPath, product, [
  'ProductSchemaVersion',
  'product_v1_schema',
  'product_v2_schema',
  'PRODUCT_EVENT_DOMAIN_V1',
  'PRODUCT_EVENT_DOMAIN_V2',
]);

const documentPath = 'crates/rustok-index/docs/m7-product-sales-channel-relation-admission.md';
const document = requireMarkers(documentPath, [
  'Status: `canonical_source_and_freshness_watermark_complete_runtime_evidence_pending`',
  'current Product Index graph contains the Product-to-SalesChannel link',
  '`product_sales_channel_index_relation_snapshots`',
  '`product_sales_channel_index_relation_freshness_snapshots`',
  '`channel_index_identity_generations`',
  '`product_index_graph_projection_snapshots.projection_epoch`',
  '`sales_channel_ids`',
  '`sales_channels` `IndexLink`',
  'Canonical Product replay/absence freshness gate: source complete',
]);
forbidMarkers(documentPath, document, [
  'Product v1',
  'Product v2',
  'Product v3',
  'future Product',
  'does not yet add',
]);

requireMarkers('scripts/verify/verify-index-query-contract.mjs', [
  "'verify-index-product-channel-relation-admission.mjs'",
  "'verify-index-product-channel-relation-freshness.mjs'",
]);

console.log('[verify-index-product-channel-relation-admission] relation and freshness admission verified');

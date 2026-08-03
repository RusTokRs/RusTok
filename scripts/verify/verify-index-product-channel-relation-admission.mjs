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

requireMarkers('crates/rustok-distribution/src/product_index/mod.rs', [
  'pub(crate) mod relation_admission;',
]);

const admissionPath =
  'crates/rustok-distribution/src/product_index/relation_admission.rs';
const admission = requireMarkers(admissionPath, [
  'PRODUCT_SALES_CHANNEL_RELATION_EVENT_DOMAIN_V1',
  'rustok-distribution.product-sales-channel-relation-v1',
  'ProductSalesChannelRelationEpoch',
  'ProductSalesChannelRelationSnapshot',
  'ProductSalesChannelRelationAdmission::Initial',
  'ProductSalesChannelRelationAdmission::Retry',
  'ProductSalesChannelRelationAdmission::Advanced',
  'SameEpochMembershipChanged',
  'EpochRegressed',
  'NilTenantId',
  'NilProductId',
  'ScopeChanged',
  'channel_ids.sort_unstable();',
  'channel_ids.windows(2)',
  'derive_index_source_event_id(',
  'Some(&locale)',
  'epoch.get()',
  'product_sales_channel_relation_canonical_membership_and_retry_identity_are_stable',
  'product_sales_channel_relation_change_requires_a_strictly_larger_epoch',
  'product_sales_channel_relation_invalid_identity_and_duplicates_fail_closed',
  'product_sales_channel_relation_empty_membership_is_valid_but_scope_cannot_change',
  'other_tenant',
  'other_product',
]);

for (const forbidden of [
  'SystemTime',
  'Instant',
  'DefaultHasher',
  'wrapping_',
  'saturating_',
  'product_revision.max',
  'channel_revision.max',
  'Utc::now',
  'CURRENT_TIMESTAMP',
]) {
  if (admission.includes(forbidden)) {
    fail(`${admissionPath} contains forbidden revision derivation ${forbidden}`);
  }
}

const graphPath = 'crates/rustok-distribution/src/product_index/graph.rs';
const graph = read(graphPath);
for (const forbidden of [
  'sales_channel',
  'link_name("channels")',
  'product_sales_channel_relation_epoch',
]) {
  if (graph.includes(forbidden)) {
    fail(
      `${graphPath} wires Product-to-SalesChannel links before durable epoch admission: ${forbidden}`,
    );
  }
}

const documentPath =
  'crates/rustok-index/docs/m7-product-sales-channel-relation-admission.md';
const document = requireMarkers(documentPath, [
  'Status: `source_contract_complete_persistence_and_wiring_pending`',
  'Rechecked on 2026-08-03 against current `main`',
  'correctly keeps durable',
  'Product-to-SalesChannel relations open',
  'must therefore not be derived with `max(product_revision,',
  '`ProductSalesChannelRelationEpoch`',
  '`ProductSalesChannelRelationSnapshot`',
  'exact non-nil tenant, non-nil',
  'an identical epoch is accepted only for an identical retry',
  'changed membership under the same epoch fails closed',
  'durable epoch storage for the exact tenant/Product relation identity',
  'a registered relation event descriptor',
  'generic mutation-event acknowledgement and source timeout substrates now exist',
  'This slice does not add a Product-to-SalesChannel `IndexLink`',
  'The canonical M7 Product/Variant/Channel link item remains open',
  'Execution is maintainer-owned',
]);
if (document.includes('PR #2793 currently owns')) {
  fail(`${documentPath} retains obsolete implementation-plan ownership text`);
}

requireMarkers('crates/rustok-index/docs/implementation-plan.md', [
  'durable Product-to-SalesChannel relations',
  'bounded scalar projection until the owner has a durable relational contract',
]);
requireMarkers('crates/rustok-index/src/application/mutation_event.rs', [
  'pub struct IndexMutationEventCatalog',
  'pub struct IndexMutationEventWorker',
]);
requireMarkers('crates/rustok-index/src/application/source_timeout.rs', [
  'const DEFAULT_INDEX_SOURCE_CALL_TIMEOUT: Duration = Duration::from_secs(30);',
  'index_source_scan_timeout',
  'index_source_load_timeout',
]);

console.log('[verify-index-product-channel-relation-admission] OK');

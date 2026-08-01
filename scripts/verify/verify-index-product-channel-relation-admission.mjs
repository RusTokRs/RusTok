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
  'NilProductId',
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

requireMarkers(
  'crates/rustok-index/docs/m7-product-sales-channel-relation-admission.md',
  [
    'Status: `source_contract_complete_persistence_and_wiring_pending`',
    'Rechecked against `main` at `e2c69b022b5380ba27eb3583688d154cb7a20d39`',
    'PR #2793 currently owns its M6/M7 actualization',
    'must therefore not be derived with `max(product_revision,',
    '`ProductSalesChannelRelationEpoch`',
    '`ProductSalesChannelRelationSnapshot`',
    'exact non-nil tenant, non-nil',
    'an identical epoch is accepted only for an identical retry',
    'changed membership under the same epoch fails closed',
    'durable epoch storage for the exact tenant/Product relation identity',
    'This slice does not add a Product-to-SalesChannel `IndexLink`',
    'The canonical M7 Product/Variant/Channel link item remains open',
    'Execution is maintainer-owned',
  ],
);

console.log('[verify-index-product-channel-relation-admission] OK');

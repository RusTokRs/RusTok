#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const read = (relative) => fs.readFileSync(path.join(root, relative), 'utf8');
const fail = (message) => {
  console.error(`[verify-index-product-storefront-channel-scope-policy] ${message}`);
  process.exit(1);
};
const requireMarkers = (relative, markers) => {
  const source = read(relative);
  for (const marker of markers) {
    if (!source.includes(marker)) fail(`${relative} is missing ${marker}`);
  }
  return source;
};

const ownerHelpers = requireMarkers('crates/rustok-product/src/services/catalog/helpers.rs', [
  'pub fn product_channel_visibility_condition(',
  'None => Condition::all().add(Expr::cust(',
  "COALESCE(metadata #> '{channel_visibility,allowed_channel_slugs}', '[]'::jsonb) = '[]'::jsonb",
]);

const resolver = requireMarkers('crates/rustok-distribution/src/product_index/channel_relation_resolver.rs', [
  'ProductChannelVisibility::Unrestricted => (',
  'SELECT id FROM channels WHERE tenant_id = $1 ORDER BY id ASC LIMIT $2',
  'ProductChannelVisibility::Restricted(slugs)',
]);

const productBridgePath = 'crates/rustok-distribution/src/product_index/product.rs';
const productBridge = requireMarkers(productBridgePath, [
  'assert_eq!(schema.fields.len(), 15);',
  'many_field("sales_channel_ids", IndexValueType::Uuid, true, true)',
  'SchemaVersion::new(PRODUCT_SCHEMA_ROUTING_KEY)',
]);
if (productBridge.includes('SchemaVersion::new(5)')) {
  fail(`${productBridgePath} must not invent a replacement schema merely to approximate channel-less visibility`);
}

const executorPath = 'crates/rustok-distribution/src/product_index/storefront_shadow_executor.rs';
const executor = requireMarkers(executorPath, [
  'pub(crate) enum ProductStorefrontIndexChannelScopeDecision',
  'ShadowEligible { public_channel_id: Uuid }',
  'OwnerNativeChannelLess',
  'ChannelLessOwnerNative',
  'pub(crate) fn classify_product_storefront_index_channel_scope(',
  '(None, None) => Ok(ProductStorefrontIndexChannelScopeDecision::OwnerNativeChannelLess)',
  '(Some(_), Some(public_channel_id)) if !public_channel_id.is_nil()',
  'Err(ProductStorefrontIndexShadowProjectionError::PublicChannelIdentityUnavailable)',
  'return Err(ProductStorefrontIndexShadowProjectionError::ChannelLessOwnerNative);',
  'channel_scope_distinguishes_owner_native_channel_less_from_invalid_identity',
]);
for (const forbidden of [
  'UNRESTRICTED_CHANNEL_SENTINEL',
  'CHANNEL_LESS_SENTINEL',
  'infer_unrestricted_from_channel_ids',
]) {
  if (executor.includes(forbidden) || productBridge.includes(forbidden) || resolver.includes(forbidden)) {
    fail(`channel-less policy must not infer or fabricate visibility membership: ${forbidden}`);
  }
}

requireMarkers('crates/rustok-distribution/src/product_index/storefront_shadow.rs', [
  'PublicChannelRequired',
  'FilterExpr::Contains(',
  'root_field("sales_channel_ids")?',
]);

console.log('[verify-index-product-storefront-channel-scope-policy] channel-less Storefront remains owner-native on Product key 4; channel-scoped shadow requires a trusted slug/UUID identity');

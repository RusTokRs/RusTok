#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const read = (relative) => fs.readFileSync(path.join(root, relative), 'utf8');
const fail = (message) => {
  console.error(`[verify-index-product-storefront-deep-page-policy] ${message}`);
  process.exit(1);
};
const requireMarkers = (relative, markers) => {
  const source = read(relative);
  for (const marker of markers) {
    if (!source.includes(marker)) fail(`${relative} is missing ${marker}`);
  }
  return source;
};

const ownerPath = 'crates/rustok-product/src/services/catalog/queries.rs';
const owner = requireMarkers(ownerPath, [
  'if page == 0 || per_page == 0 || per_page > 48',
  'let offset = (page.saturating_sub(1)) * per_page;',
  'query.offset(offset).limit(per_page).all(&self.db).await?',
]);
const modernList = owner.slice(
  owner.indexOf('pub async fn list_published_products_with_query('),
  owner.indexOf('pub(crate) async fn list_legacy_storefront_products_with_locale_fallback('),
);
if (modernList.includes('10_000')) {
  fail(`${ownerPath} must not silently narrow owner-valid modern Storefront depth to the Index bound`);
}

const builderPath = 'crates/rustok-distribution/src/product_index/storefront_shadow.rs';
const builder = requireMarkers(builderPath, [
  'const MAX_INDEX_OFFSET_DEPTH: u64 = 10_000;',
  'if offset > MAX_INDEX_OFFSET_DEPTH',
  'ProductStorefrontIndexShadowError::OffsetTooDeep',
]);

const executorPath = 'crates/rustok-distribution/src/product_index/storefront_shadow_executor.rs';
const executor = requireMarkers(executorPath, [
  'const MAX_INDEX_OFFSET_DEPTH: u64 = 10_000;',
  'pub(crate) enum ProductStorefrontIndexPageScopeDecision',
  'ShadowEligible { offset: u64 }',
  'OwnerNativeDeepPage { offset: u64 }',
  'DeepPageOwnerNative { offset: u64 }',
  'pub(crate) fn classify_product_storefront_index_page_scope(',
  'checked_sub(1)',
  'checked_mul(query.per_page)',
  'if offset > MAX_INDEX_OFFSET_DEPTH',
  'classify_product_storefront_index_page_scope(&query)',
  'ProductStorefrontIndexShadowProjectionError::DeepPageOwnerNative',
  'page_scope_distinguishes_shallow_from_owner_native_deep_pages',
  'ShadowEligible { offset: 9_984 }',
  'OwnerNativeDeepPage { offset: 10_032 }',
]);

const builderMatch = builder.match(/MAX_INDEX_OFFSET_DEPTH: u64 = ([\d_]+);/);
const executorMatch = executor.match(/MAX_INDEX_OFFSET_DEPTH: u64 = ([\d_]+);/);
if (!builderMatch || !executorMatch || builderMatch[1] !== executorMatch[1]) {
  fail('shadow builder and executor page classifier must retain the same Index offset depth');
}

for (const forbidden of [
  '.min(MAX_INDEX_OFFSET_DEPTH)',
  'offset = MAX_INDEX_OFFSET_DEPTH',
  'Pagination::Cursor',
]) {
  if (executor.includes(forbidden)) {
    fail(`${executorPath} must preserve owner pagination without clamp/rewrite: ${forbidden}`);
  }
}
if (/query\.page\s*=[^=]/.test(executor) || /query\.per_page\s*=[^=]/.test(executor)) {
  fail(`${executorPath} must preserve owner pagination without clamp/rewrite`);
}

const ownerPosition = executor.indexOf('list_filtered_published_products(');
const classifierPosition = executor.indexOf('classify_product_storefront_index_page_scope(&query)');
const schemaReadPosition = executor.indexOf('.schema_read_port()');
if (
  ownerPosition < 0 ||
  classifierPosition < 0 ||
  schemaReadPosition < 0 ||
  ownerPosition > classifierPosition ||
  classifierPosition > schemaReadPosition
) {
  fail('deep-page classification must happen after owner success and before projected schema/EAV reads');
}

console.log('[verify-index-product-storefront-deep-page-policy] shallow pages stay shadow-eligible; owner-valid offsets above 10000 stay typed owner-native without clamp/rewrite');

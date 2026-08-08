#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const read = (relative) => fs.readFileSync(path.join(root, relative), 'utf8');
const fail = (message) => {
  console.error(`[verify-index-product-storefront-shadow-executor] ${message}`);
  process.exit(1);
};
const requireMarkers = (relative, markers) => {
  const source = read(relative);
  for (const marker of markers) {
    if (!source.includes(marker)) fail(`${relative} is missing ${marker}`);
  }
  return source;
};

const executorPath = 'crates/rustok-distribution/src/product_index/storefront_shadow_executor.rs';
const source = requireMarkers(executorPath, [
  'pub(crate) struct ProductStorefrontIndexShadowExecutor',
  'product: ProductCatalogReadRuntime',
  'index: SharedIndexQueryRuntime',
  'pub(crate) struct ProductStorefrontIndexShadowExecution',
  'pub(crate) authoritative: StorefrontProductList',
  'pub(crate) projected: Result<IndexQueryPage, ProductStorefrontIndexShadowProjectionError>',
  'pub(crate) public_projected:',
  'Option<Result<IndexQueryPage, ProductStorefrontIndexPublicProjectionError>>',
  'pub(crate) enum ProductStorefrontIndexChannelScopeDecision',
  'OwnerNativeChannelLess',
  'ChannelLessOwnerNative',
  'pub(crate) enum ProductStorefrontIndexPageScopeDecision',
  'OwnerNativeDeepPage { offset: u64 }',
  'DeepPageOwnerNative { offset: u64 }',
  'list_filtered_published_products(',
  'let projected = self',
  '.execute_projected(',
  'let public_projected = projected',
  '.as_ref()',
  '.ok()',
  '.cloned()',
  '.map(project_product_storefront_index_page);',
  'let comparison = projected',
  'compare_owner_and_index(&authoritative, projected)',
  'classify_product_storefront_index_channel_scope(',
  'classify_product_storefront_index_page_scope(&query)',
  '.schema_read_port()',
  '.resolve_storefront_attribute_filters(',
  'build_product_storefront_index_shadow_query(',
  '.execute_localized_query(index_query)',
  'identities_match: authoritative_ids == projected_ids',
  'projected.exact_count == Some(authoritative.total)',
  'projected.has_more == authoritative.has_next',
]);

const ownerPosition = source.indexOf('list_filtered_published_products(');
const projectedPosition = source.indexOf('let projected = self');
const publicPosition = source.indexOf('let public_projected = projected');
const comparisonPosition = source.indexOf('let comparison = projected');
if (
  ownerPosition < 0 ||
  projectedPosition <= ownerPosition ||
  publicPosition <= projectedPosition ||
  comparisonPosition <= projectedPosition
) {
  fail('owner success must precede raw Index projection, and raw page must precede public projection/comparison');
}

const rawStart = source.indexOf('async fn execute_projected(');
const compareStart = source.indexOf('fn compare_owner_and_index(');
if (rawStart < 0 || compareStart <= rawStart) fail('raw projected execution boundary is missing');
if (source.slice(rawStart, compareStart).includes('project_product_storefront_index_page')) {
  fail('Product public placeholder transform must not run inside raw Index execution');
}

for (const forbidden of [
  'DatabaseConnection',
  'CatalogService::new',
  'ProductCatalogSchemaService::new',
  'PostgresIndexQueryPort',
  'query_all(',
  'query_one(',
  'CHANNEL_LESS_SENTINEL',
  'UNRESTRICTED_CHANNEL_SENTINEL',
  '.min(MAX_INDEX_OFFSET_DEPTH)',
  'Pagination::Cursor',
]) {
  if (source.includes(forbidden)) fail(`${executorPath} must compose selected ports and preserve owner request semantics; found ${forbidden}`);
}

requireMarkers('crates/rustok-distribution/src/product_index/storefront_projection.rs', [
  'pub(crate) fn project_product_storefront_index_page(',
  'const UNTITLED_PRODUCT: &str = "Untitled product";',
  'apply_string_placeholder(item, "handle", "")?;',
]);
requireMarkers('crates/rustok-distribution/src/product_index/mod.rs', [
  'mod storefront_projection;',
  'ProductStorefrontIndexPublicProjectionError',
  'project_product_storefront_index_page',
  'mod storefront_shadow_executor;',
  'ProductStorefrontIndexShadowExecutor',
  'ProductStorefrontIndexPageScopeDecision',
]);

const mountedPath = 'crates/rustok-product/storefront/src/transport/catalog_list_native.rs';
const mounted = requireMarkers(mountedPath, [
  'CatalogService::new(runtime_ctx.db_clone(), event_bus)',
  '.list_published_products_with_query(',
]);
for (const forbidden of [
  'ProductStorefrontIndexShadowExecutor',
  'execute_localized_query',
  'project_product_storefront_index_page',
]) {
  if (mounted.includes(forbidden)) fail(`${mountedPath} must remain owner-native; found ${forbidden}`);
}

console.log('[verify-index-product-storefront-shadow-executor] owner-first raw Index evidence and derived post-page Product public projection are source-locked; mounted Storefront remains owner-native');

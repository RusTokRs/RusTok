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
  'list_filtered_published_products(',
  'let projected = self',
  '.schema_read_port()',
  '.resolve_storefront_attribute_filters(',
  'build_product_storefront_index_shadow_query(',
  '.execute_localized_query(index_query)',
  'PublicChannelIdentityUnavailable',
  'SchemaReadPortUnavailable',
  'compare_owner_and_index(&authoritative, projected)',
  'identities_match: authoritative_ids == projected_ids',
  'projected.exact_count == Some(authoritative.total)',
  'projected.has_more == authoritative.has_next',
]);

const ownerPosition = source.indexOf('list_filtered_published_products(');
const projectedPosition = source.indexOf('let projected = self');
if (ownerPosition < 0 || projectedPosition < 0 || ownerPosition > projectedPosition) {
  fail('authoritative Product owner list must execute before any Index shadow work');
}

for (const forbidden of [
  'DatabaseConnection',
  'CatalogService::new',
  'ProductCatalogSchemaService::new',
  'PostgresIndexQueryPort',
  'query_all(',
  'query_one(',
]) {
  if (source.includes(forbidden)) fail(`${executorPath} must compose selected ports only; found ${forbidden}`);
}

requireMarkers('crates/rustok-distribution/src/product_index/mod.rs', [
  'mod storefront_shadow_executor;',
  'ProductStorefrontIndexShadowExecutor',
  'ProductStorefrontIndexShadowExecution',
  'ProductStorefrontIndexShadowComparison',
]);

const mountedPath = 'crates/rustok-product/storefront/src/transport/catalog_list_native.rs';
const mounted = requireMarkers(mountedPath, [
  'CatalogService::new(runtime_ctx.db_clone(), event_bus)',
  '.list_published_products_with_query(',
]);
for (const forbidden of ['ProductStorefrontIndexShadowExecutor', 'execute_localized_query']) {
  if (mounted.includes(forbidden)) fail(`${mountedPath} must remain owner-native; found ${forbidden}`);
}

console.log('[verify-index-product-storefront-shadow-executor] owner-first non-serving shadow execution is source-locked; mounted Storefront remains owner-native');

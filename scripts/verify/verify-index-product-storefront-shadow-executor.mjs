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
  'pub(crate) tag_hydration:',
  'list_filtered_published_products(',
  'let projected = self',
  '.execute_projected(',
  'let public_projected = projected',
  '.map(project_product_storefront_index_page);',
  'let tag_hydration = match projected.as_ref()',
  'self.hydrate_projected_tags(context, fallback_locale, projected)',
  'pub(crate) async fn hydrate_projected_tags(',
  '.storefront_tag_read_port()',
  '.hydrate_storefront_product_tags(',
  'pub(crate) async fn execute_projected(',
  '.schema_read_port()',
  '.resolve_storefront_attribute_filters(',
  'build_product_storefront_index_shadow_query(',
  '.execute_localized_query(index_query)',
  'let comparison = projected',
  'compare_owner_and_index(&authoritative, projected)',
]);

const ownerPosition = source.indexOf('list_filtered_published_products(');
const projectedPosition = source.indexOf('let projected = self');
const publicPosition = source.indexOf('let public_projected = projected');
const hydrationPosition = source.indexOf('let tag_hydration = match projected.as_ref()');
if (
  ownerPosition < 0 ||
  projectedPosition <= ownerPosition ||
  publicPosition <= projectedPosition ||
  hydrationPosition <= projectedPosition
) {
  fail('owner success must precede raw Index evidence, and post-page enrichment must follow it');
}

const rawStart = source.indexOf('pub(crate) async fn execute_projected(');
const compareStart = source.indexOf('fn compare_owner_and_index(');
if (rawStart < 0 || compareStart <= rawStart) fail('crate-private raw projected execution boundary is missing');
const rawExecution = source.slice(rawStart, compareStart);
for (const forbidden of [
  'project_product_storefront_index_page',
  'hydrate_storefront_product_tags',
  'storefront_tag_read_port()',
]) {
  if (rawExecution.includes(forbidden)) {
    fail(`post-page enrichment must not run inside raw Index execution: ${forbidden}`);
  }
}

for (const forbidden of [
  'DatabaseConnection',
  'CatalogService::new',
  'ProductCatalogSchemaService::new',
  'PostgresIndexQueryPort',
  'TaxonomyService',
  'product_tag::',
  'query_all(',
  'query_one(',
  'CHANNEL_LESS_SENTINEL',
  'UNRESTRICTED_CHANNEL_SENTINEL',
  '.min(MAX_INDEX_OFFSET_DEPTH)',
  'Pagination::Cursor',
  'tokio::time::timeout',
  'ProductStorefrontIndexServingBudgetDecision',
]) {
  if (source.includes(forbidden)) fail(`${executorPath} must remain the unbudgeted evidence executor; found ${forbidden}`);
}

const budgetedPath = 'crates/rustok-distribution/src/product_index/storefront_budgeted_execution.rs';
requireMarkers(budgetedPath, [
  'ProductStorefrontIndexBudgetedProjectionExecutor',
  'ProductStorefrontIndexServingBudgetDecision::Eligible',
  'self.shadow.execute_projected(',
  '.hydrate_projected_tags(tag_context, fallback_locale, projected)',
  'use tokio::time::timeout;',
]);

requireMarkers('crates/rustok-product/src/storefront_tag_read_port.rs', [
  'pub trait ProductStorefrontTagReadPort',
  'MAX_STOREFRONT_TAG_HYDRATION_PRODUCTS: usize = 48',
  '.load_product_tag_map(',
]);
requireMarkers('crates/rustok-distribution/src/product_index/storefront_projection.rs', [
  'pub(crate) fn project_product_storefront_index_page(',
  'value(&projected.items[0], "tag_ids")',
]);
requireMarkers('crates/rustok-distribution/src/product_index/mod.rs', [
  'ProductStorefrontIndexTagHydrationError',
  'ProductStorefrontIndexBudgetedProjectionExecutor',
]);

const mountedPath = 'crates/rustok-product/storefront/src/transport/catalog_list_native.rs';
const mounted = requireMarkers(mountedPath, [
  'CatalogService::new(runtime_ctx.db_clone(), event_bus)',
  '.list_published_products_with_query(',
]);
for (const forbidden of [
  'ProductStorefrontIndexShadowExecutor',
  'ProductStorefrontIndexBudgetedProjectionExecutor',
  'execute_localized_query',
  'project_product_storefront_index_page',
  'hydrate_storefront_product_tags',
]) {
  if (mounted.includes(forbidden)) fail(`${mountedPath} must remain owner-native; found ${forbidden}`);
}

console.log('[verify-index-product-storefront-shadow-executor] owner-first evidence executor exposes crate-private post-owner phases to a separate budgeted adapter while mounted Storefront remains owner-native');

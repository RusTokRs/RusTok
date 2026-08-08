#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const read = (relative) => fs.readFileSync(path.join(root, relative), 'utf8');
const fail = (message) => {
  console.error(`[verify-index-product-storefront-parity-gate] ${message}`);
  process.exit(1);
};
const requireMarkers = (relative, markers) => {
  const source = read(relative);
  for (const marker of markers) {
    if (!source.includes(marker)) fail(`${relative} is missing ${marker}`);
  }
  return source;
};

const storefrontPath = 'crates/rustok-product/storefront/src/transport/catalog_list_native.rs';
const storefront = requireMarkers(storefrontPath, [
  'CatalogService::new(runtime_ctx.db_clone(), event_bus)',
  '.list_published_products_with_query(',
]);
for (const forbidden of ['rustok_index', 'SharedIndexQueryRuntime', 'execute_localized_query']) {
  if (storefront.includes(forbidden)) fail(`${storefrontPath} contains forbidden marker ${forbidden}`);
}

const ownerPath = 'crates/rustok-product/src/services/catalog/queries.rs';
const owner = requireMarkers(ownerPath, [
  'ProductStatus::Active',
  'Column::PublishedAt.is_not_null()',
  'product_channel_visibility_condition(',
  'attribute_filters::load_catalog_attribute_filter_conditions(',
  'product_title_search_condition(',
  'let total = query.clone().count(&self.db).await?',
  'pick_product_translation(items.as_slice(), locale, fallback_locale)',
  'let pattern = format!("%{search}%");',
  'pt.title LIKE $1',
  '.order_by_asc(entities::product::Column::Id)',
  '.order_by_desc(entities::product::Column::Id)',
]);
const titleSearch = owner.slice(owner.indexOf('fn product_title_search_condition('));
if (titleSearch.includes('pt.locale')) fail(`${ownerPath} title search became locale-scoped`);

requireMarkers('crates/rustok-product/src/catalog_schema_read_port.rs', [
  'ProductStorefrontAttributeFilterResolutionRequest',
  'async fn resolve_storefront_attribute_filters(',
  '"product.storefront_attribute_filter_resolution_unavailable"',
]);
requireMarkers('crates/rustok-product/src/services/catalog_attribute_terms.rs', [
  'pub enum ProductAttributeTermExpr',
  'pub struct ProductResolvedAttributeFilter',
  'product_attribute_localized_text_expr(',
]);
requireMarkers('crates/rustok-distribution/src/product_index/storefront_shadow_executor.rs', [
  'pub(crate) struct ProductStorefrontIndexShadowExecutor',
  'list_filtered_published_products(',
  '.resolve_storefront_attribute_filters(',
  '.execute_localized_query(index_query)',
  'pub(crate) authoritative: StorefrontProductList',
  'pub(crate) projected: Result<IndexQueryPage, ProductStorefrontIndexShadowProjectionError>',
]);
requireMarkers('crates/rustok-distribution/src/product_index/storefront_shadow_postgres_tests.rs', [
  'RUSTOK_PRODUCT_STOREFRONT_EQUIVALENCE_DATABASE_URL',
  'SchemaVersion::new(PRODUCT_SCHEMA_ROUTING_KEY)',
  'ProductSalesChannelRelationResolver',
  'ProductStorefrontIndexShadowExecutor',
  'assert_eq!(owner_c.title, "Untitled product");',
  'assert_eq!(projected_string(index_c, "title")?, None);',
]);

const productIndexPath = 'crates/rustok-distribution/src/product_index/product.rs';
const productIndex = requireMarkers(productIndexPath, [
  'assert_eq!(schema.fields.len(), 15);',
  'JOIN product_translations t',
  'SchemaVersion::new(PRODUCT_SCHEMA_ROUTING_KEY)',
  'derive_index_schema_source_event_id(',
]);
if (productIndex.includes('SchemaVersion::new(3)')) fail(`${productIndexPath} restored historical key 3`);

requireMarkers('crates/rustok-index/docs/m7-product-storefront-parity-gate.md', [
  'Status: `core_postgres_packet_source_complete_execution_and_eav_pending`',
  'Mounted Storefront remains owner-native',
  'Core PostgreSQL owner-vs-shadow packet — source complete, execution pending',
  'routing key `4`',
  'third-locale',
  'placeholder',
  'EAV parity',
]);
requireMarkers('crates/rustok-index/docs/m7-product-attribute-term-contract.md', [
  'Status: `source_complete_materialized_rebuild_pending`',
  '`requested-value OR (NOT requested-present AND fallback-value)`',
]);

console.log('[verify-index-product-storefront-parity-gate] Storefront remains owner-native; current-key core owner-vs-shadow PostgreSQL packet is retained in source while execution, EAV, placeholder and policy gates remain pending');

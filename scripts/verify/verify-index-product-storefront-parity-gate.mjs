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
const forbidMarkers = (relative, source, markers) => {
  for (const marker of markers) {
    if (source.includes(marker)) fail(`${relative} contains forbidden marker ${marker}`);
  }
};

const storefrontPath = 'crates/rustok-product/storefront/src/transport/catalog_list_native.rs';
const storefront = requireMarkers(storefrontPath, [
  'StorefrontProductListQuery::try_from_transport_with_attribute_filters(',
  '.with_pagination(1, 12)',
  'CatalogService::new(runtime_ctx.db_clone(), event_bus)',
  '.list_published_products_with_query(',
  'public_channel_slug.as_deref()',
  'seller_id: item.seller_id',
  'tags: item.tags',
  'created_at: item.created_at.to_rfc3339()',
  'published_at: item.published_at.map(|value| value.to_rfc3339())',
]);
forbidMarkers(storefrontPath, storefront, [
  'rustok_index',
  'SharedIndexQueryRuntime',
  'IndexQuery',
  'materialize_postgres_index_query_runtime',
]);

const typesPath = 'crates/rustok-product/src/services/catalog/types.rs';
requireMarkers(typesPath, [
  'pub enum StorefrontProductSortBy',
  'PublishedAt',
  'CreatedAt',
  'pub struct ProductAttributeFilter',
  'const MAX_ATTRIBUTE_FILTERS: usize = 8',
  'pub struct StorefrontProductListQuery',
  'pub attribute_filters: Vec<ProductAttributeFilter>',
  'pub struct StorefrontProductListItem',
  'pub seller_id: Option<String>',
  'pub tags: Vec<String>',
  'pub created_at: chrono::DateTime<chrono::Utc>',
  'pub published_at: Option<chrono::DateTime<chrono::Utc>>',
]);

const ownerQueryPath = 'crates/rustok-product/src/services/catalog/queries.rs';
const ownerQuery = requireMarkers(ownerQueryPath, [
  'ProductStatus::Active',
  'Column::PublishedAt.is_not_null()',
  'product_channel_visibility_condition(',
  'Column::PrimaryCategoryId.eq(category_id)',
  'product_title_search_condition(',
  'attribute_filters::load_catalog_attribute_filter_conditions(',
  'let total = query.clone().count(&self.db).await?',
  'pick_product_translation(items.as_slice(), locale, fallback_locale)',
  'fn product_title_search_condition(',
  'FROM product_translations pt',
  'pt.product_id = products.id',
  'pt.title LIKE $1',
]);
const titleSearch = ownerQuery.slice(ownerQuery.indexOf('fn product_title_search_condition('));
if (titleSearch.includes('pt.locale')) {
  fail(`${ownerQueryPath} title search became locale-scoped; update parity architecture in the same PR`);
}

const productIndexPath = 'crates/rustok-distribution/src/product_index/product.rs';
const productIndex = requireMarkers(productIndexPath, [
  'assert_eq!(schema.fields.len(), 15);',
  'JOIN product_translations t',
  't.locale',
  'SchemaVersion::new(PRODUCT_SCHEMA_ROUTING_KEY)',
  'derive_index_schema_source_event_id(',
  'many_field("attribute_terms", IndexValueType::String, false, true)?',
]);
forbidMarkers(productIndexPath, productIndex, [
  'SchemaVersion::new(3)',
  'derive_index_source_event_id(',
  'product_v1_schema',
  'product_v2_schema',
  'COALESCE(requested_translation',
]);

requireMarkers('crates/rustok-distribution/src/product_index/absence.rs', [
  'translation.locale = $3',
  'return Ok(None);',
  'SchemaVersion::new(PRODUCT_SCHEMA_ROUTING_KEY)',
]);
requireMarkers('crates/rustok-index/src/infrastructure/postgres/schema_registration.rs', [
  'VersionConflict { reference: SchemaRef }',
  'pub async fn register_current(',
  'retire_lower_active_schemas(',
  "status = 'retired'",
]);

requireMarkers('crates/rustok-index/docs/m7-product-storefront-parity-gate.md', [
  'Status: `localized_compiler_decoder_source_complete_runtime_and_evidence_pending`',
  'does **not** restrict that search row to the requested or fallback locale',
  'owner list can still return the Product using its fallback translation',
  'one Index entity for each physically stored `product_translations.locale`',
  'A scalar substring/LIKE operator alone cannot close Storefront parity',
  'localized-entity identity fold',
  '`localized_projection_fields`',
  '`SchemaRegistry::compile_postgres_localized_page_query`',
  '`SchemaRegistry::decode_postgres_localized_query_page`',
  'The public query runtime still has no `execute_localized_query` method.',
  'm7-product-storefront-localized-query-architecture.md',
  'Do **not** add another Product routing key merely to patch this query semantic.',
  'actualize retained Product PostgreSQL packets',
  'Storefront must continue to execute `CatalogService::list_published_products_with_query`',
]);
requireMarkers('crates/rustok-index/docs/m7-product-attribute-term-contract.md', [
  'Status: `source_complete_materialized_rebuild_pending`',
  '`requested-value OR (NOT requested-present AND fallback-value)`',
]);

console.log('[verify-index-product-storefront-parity-gate] Storefront remains owner-native while localized runtime/evidence are pending');

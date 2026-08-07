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
  'pub enum StorefrontProductSortDirection',
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
requireMarkers(ownerQueryPath, [
  'ProductStatus::Active',
  'Column::PublishedAt.is_not_null()',
  'product_channel_visibility_condition(',
  'Column::PrimaryCategoryId.eq(category_id)',
  'product_title_search_condition(',
  'attribute_filters::load_catalog_attribute_filter_conditions(',
  'StorefrontProductSortBy::PublishedAt',
  'StorefrontProductSortBy::CreatedAt',
  '.order_by_asc(entities::product::Column::Id)',
  '.order_by_desc(entities::product::Column::Id)',
  'let total = query.clone().count(&self.db).await?',
  'pick_product_translation(items.as_slice(), locale, fallback_locale)',
  'load_product_tag_map(tenant_id, &products, locale, Some(fallback_locale))',
]);

const productIndexPath = 'crates/rustok-distribution/src/product_index/product.rs';
const productIndex = read(productIndexPath);
const schemaStart = productIndex.indexOf('fn product_schema()');
const schemaEnd = productIndex.indexOf('fn validated_schema(', schemaStart);
if (schemaStart < 0 || schemaEnd < 0 || schemaEnd <= schemaStart) {
  fail(`${productIndexPath} canonical Product schema block is missing`);
}
const productSchema = productIndex.slice(schemaStart, schemaEnd);
for (const marker of [
  'scalar_field("id"',
  'scalar_field("status"',
  'scalar_field("title"',
  'scalar_field("handle"',
  'scalar_field("description"',
  'scalar_field("seller_id"',
  'scalar_field("vendor"',
  'scalar_field("product_type"',
  '"primary_category_id"',
  'many_field("tag_ids"',
  'scalar_field("created_at"',
  'scalar_field("published_at"',
  'many_field("attribute_terms"',
  'many_field("variant_ids"',
  'many_field("sales_channel_ids"',
  'name: link_name("variants")?',
  'name: link_name("sales_channels")?',
]) {
  if (!productSchema.includes(marker)) fail(`${productIndexPath} schema is missing ${marker}`);
}
if (!productIndex.includes('assert_eq!(schema.fields.len(), 15);')) {
  fail(`${productIndexPath} current single Product field-count assertion is not 15`);
}
forbidMarkers(productIndexPath, productIndex, [
  'SchemaVersion::new(3)',
  'derive_index_source_event_id(',
  'ProductSchemaVersion',
  'product_v1_schema',
  'product_v2_schema',
]);
requireMarkers(productIndexPath, [
  'SchemaVersion::new(PRODUCT_SCHEMA_ROUTING_KEY)',
  'derive_index_schema_source_event_id(',
  "COALESCE(tags.tag_ids, '[]'::jsonb) AS tag_ids",
  "COALESCE(attributes.attribute_terms, '[]'::jsonb) AS attribute_terms",
]);

const schemaStorePath = 'crates/rustok-index/src/infrastructure/postgres/schema_registration.rs';
requireMarkers(schemaStorePath, [
  'VersionConflict { reference: SchemaRef }',
  'NonMonotonicVersion {',
  'schema {reference} is already registered with another contract',
  'existing.fingerprint != fingerprint.to_string() || existing.schema_json != *schema_json',
  'pub async fn register_current(',
  'retire_lower_active_schemas(',
  "status = 'retired'",
]);
forbidMarkers(schemaStorePath, read(schemaStorePath), [
  'DELETE FROM index_schemas',
  'DELETE FROM index_entities',
  'DELETE FROM index_links',
]);

requireMarkers('crates/rustok-index/docs/m7-product-storefront-parity-gate.md', [
  'Status: `source_complete_query_adapter_and_evidence_pending`',
  'Current Product runtime code publishes one Product schema with 15 fields and two links',
  '`seller_id`, `created_at`, `published_at`',
  '`tag_ids`',
  '`attribute_terms`',
  'Tags remain Taxonomy-owned',
  'query-adapter/evidence gated',
  'ordinary-register the current key',
  '`PostgresSchemaRegistrationStore::register_current`',
  'There is no same-key fingerprint replacement and no parallel Product v4/v5 route.',
  'Storefront must continue to execute `CatalogService::list_published_products_with_query`',
]);

requireMarkers('crates/rustok-index/docs/m7-product-attribute-term-contract.md', [
  'Status: `source_complete_materialized_rebuild_pending`',
  '`requested-value OR (NOT requested-present AND fallback-value)`',
]);

requireMarkers('crates/rustok-index/docs/m4-single-current-schema-supersession.md', [
  'Status: `source_complete_execution_pending`',
  'single-current',
  '`derive_index_schema_source_event_id`',
]);

console.log('[verify-index-product-storefront-parity-gate] Product source coverage is complete; Storefront cutover remains fail-closed on adapter, rebuild and retained parity evidence');

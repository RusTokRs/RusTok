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
  'scalar_field("vendor"',
  'scalar_field("product_type"',
  '"primary_category_id"',
  'many_field("variant_ids"',
  'many_field("sales_channel_ids"',
  'name: link_name("variants")?',
  'name: link_name("sales_channels")?',
]) {
  if (!productSchema.includes(marker)) fail(`${productIndexPath} schema is missing ${marker}`);
}
for (const missingStorefrontField of [
  'scalar_field("seller_id"',
  'many_field("tags"',
  'scalar_field("created_at"',
  'scalar_field("published_at"',
]) {
  if (productSchema.includes(missingStorefrontField)) {
    fail(`${productIndexPath} changed Storefront parity coverage without updating this gate: ${missingStorefrontField}`);
  }
}
if (!productIndex.includes('assert_eq!(schema.fields.len(), 10);')) {
  fail(`${productIndexPath} current canonical Product field-count assertion changed without parity-gate review`);
}

const schemaStorePath = 'crates/rustok-index/src/infrastructure/postgres/schema_registration.rs';
requireMarkers(schemaStorePath, [
  'VersionConflict { reference: SchemaRef }',
  'NonMonotonicVersion {',
  'schema {reference} is already registered with another contract',
  'schema version must increase for {identity}',
  'existing.fingerprint != fingerprint.to_string() || existing.schema_json != *schema_json',
  'Err(SchemaRegistrationError::VersionConflict {',
  'schema.reference.version <= latest',
]);

const parityDocPath = 'crates/rustok-index/docs/m7-product-storefront-parity-gate.md';
requireMarkers(parityDocPath, [
  'Status: `source_complete_cutover_blocked_by_contract_gap`',
  'seller_id',
  'Product `tags`',
  '`created_at` result + sort key',
  '`published_at` result + non-null published-only admission + sort key',
  'dynamic typed EAV `attribute_filters`',
  'no silent same-key fingerprint replacement',
  'no parallel v4/v5 compatibility source/route',
  'Storefront must continue to execute `CatalogService::list_published_products_with_query`',
]);

const currentPlanPath = 'crates/rustok-index/docs/implementation-plan-current-2026-08-07.md';
requireMarkers(currentPlanPath, [
  'Prove complete Storefront Product/ProductVariant/SalesChannel query parity.',
  'Move Storefront traffic only after readiness/equivalence/freshness/availability evidence passes.',
  'do not introduce a new Product schema/version to solve',
]);

console.log('[verify-index-product-storefront-parity-gate] Storefront cutover remains fail-closed on the current immutable Product Index contract');

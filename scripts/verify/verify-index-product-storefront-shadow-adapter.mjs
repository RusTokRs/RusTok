#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const read = (relative) => fs.readFileSync(path.join(root, relative), 'utf8');
const fail = (message) => {
  console.error(`[verify-index-product-storefront-shadow-adapter] ${message}`);
  process.exit(1);
};
const requireMarkers = (relative, markers) => {
  const source = read(relative);
  for (const marker of markers) {
    if (!source.includes(marker)) fail(`${relative} is missing ${marker}`);
  }
  return source;
};

const shadowPath = 'crates/rustok-distribution/src/product_index/storefront_shadow.rs';
const shadow = requireMarkers(shadowPath, [
  'pub(crate) fn build_product_storefront_index_shadow_query(',
  'resolved_attribute_filters: Vec<ProductResolvedAttributeFilter>',
  'resolved_attribute_filters_to_index(owner, resolved_attribute_filters)?',
  'owner.code.eq_ignore_ascii_case(resolved.code.as_str())',
  'fn product_term_expr_to_index(',
  'ProductAttributeTermExpr::Term(term)',
  'ProductAttributeTermExpr::And(children)',
  'ProductAttributeTermExpr::Or(children)',
  'ProductAttributeTermExpr::Not(child)',
  'ProductAttributeTermExpr::Never',
  'FilterExpr::Contains(',
  'root_field("attribute_terms")?',
  'FilterExpr::Not(Box::new(FilterExpr::IsNull(',
  'root_field("id")?',
  'ProductStorefrontIndexShadowError::PublicChannelRequired',
  'ProductStorefrontIndexShadowError::OffsetTooDeep',
  'ProductStorefrontIndexShadowError::SearchPatternTooLong',
  'ProductStorefrontIndexShadowError::AttributeFilterResolutionMismatch',
  'ProductStorefrontIndexShadowError::InvalidAttributeTermPredicate',
  'ProductStatus::Active.to_string()',
  'FilterExpr::TextLike(root_field("title")?, pattern)',
  'StorefrontProductSortBy::PublishedAt => ["published_at", "created_at"]',
  'StorefrontProductSortBy::CreatedAt => ["created_at", "published_at"]',
  'Pagination::Offset { limit, offset }',
  'include_exact_count: true',
  '.with_localized_projection_fields([root_field("title")?, root_field("handle")?])',
  '.with_identity_order_direction(direction)',
  'SchemaVersion::new(PRODUCT_SCHEMA_ROUTING_KEY)',
]);
for (const forbidden of [
  'DatabaseConnection',
  'CatalogService::new',
  'ProductCatalogSchemaService',
  'product_attribute_options::Entity',
  'product_attribute_values::Entity',
  'execute_localized_query(',
]) {
  if (shadow.includes(forbidden)) fail(`${shadowPath} must remain a pure shadow query builder; found ${forbidden}`);
}

requireMarkers('crates/rustok-product/src/catalog_schema_read_port.rs', [
  'async fn resolve_storefront_attribute_filters(',
  'ProductResolvedAttributeFilter',
]);
requireMarkers('crates/rustok-distribution/src/product_index/mod.rs', [
  'mod storefront_shadow;',
  'build_product_storefront_index_shadow_query',
  'ProductStorefrontIndexShadowError',
  'PRODUCT_SCHEMA_ROUTING_KEY: u32 = 4',
]);

const storefrontPath = 'crates/rustok-product/storefront/src/transport/catalog_list_native.rs';
const storefront = requireMarkers(storefrontPath, [
  'CatalogService::new(runtime_ctx.db_clone(), event_bus)',
  '.list_published_products_with_query(',
]);
for (const forbidden of ['rustok_index', 'SharedIndexQueryRuntime', 'execute_localized_query']) {
  if (storefront.includes(forbidden)) fail(`${storefrontPath} must remain owner-native; found ${forbidden}`);
}

console.log('[verify-index-product-storefront-shadow-adapter] shadow builder consumes only Product-owned resolved terms and remains non-serving/fail-closed');

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

const typesPath = 'crates/rustok-product/src/services/catalog/types.rs';
requireMarkers(typesPath, [
  'pub const MAX_STOREFRONT_PRODUCT_SEARCH_BYTES: usize = 1022;',
  'search: normalize_storefront_product_search(search)?',
  'pub(crate) fn validate_storefront_product_search(',
  'search.len() > MAX_STOREFRONT_PRODUCT_SEARCH_BYTES',
]);

const ownerPath = 'crates/rustok-product/src/services/catalog/queries.rs';
const owner = requireMarkers(ownerPath, [
  'ProductStatus::Active',
  'Column::PublishedAt.is_not_null()',
  'product_channel_visibility_condition(',
  'attribute_filters::load_catalog_attribute_filter_conditions(',
  'types::validate_storefront_product_search(list_query.search.as_deref())?;',
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
const ownerValidation = owner.indexOf(
  'types::validate_storefront_product_search(list_query.search.as_deref())?;',
);
const ownerSql = owner.indexOf('let mut query = entities::product::Entity::find()');
if (ownerValidation < 0 || ownerSql <= ownerValidation) {
  fail(`${ownerPath} must validate Storefront search before constructing owner SQL`);
}

requireMarkers('crates/rustok-product/src/catalog_schema_read_port.rs', [
  'ProductStorefrontAttributeFilterResolutionRequest',
  'async fn resolve_storefront_attribute_filters(',
  '"product.storefront_attribute_filter_resolution_unavailable"',
]);
requireMarkers('crates/rustok-product/src/services/catalog_attribute_terms.rs', [
  'pub enum ProductAttributeTermExpr',
  'pub struct ProductResolvedAttributeFilter',
  'product_attribute_localized_text_expr(',
  'ProductAttributeTermExpr::Never',
]);
requireMarkers('crates/rustok-distribution/src/product_index/storefront_shadow.rs', [
  'services::MAX_STOREFRONT_PRODUCT_SEARCH_BYTES',
  'if search.len() > MAX_STOREFRONT_PRODUCT_SEARCH_BYTES',
  'FilterExpr::TextLike(root_field("title")?, pattern)',
]);
requireMarkers('crates/rustok-distribution/src/product_index/storefront_shadow_executor.rs', [
  'pub(crate) struct ProductStorefrontIndexShadowExecutor',
  'list_filtered_published_products(',
  '.resolve_storefront_attribute_filters(',
  '.execute_localized_query(index_query)',
  'pub(crate) authoritative: StorefrontProductList',
]);
requireMarkers('crates/rustok-distribution/src/product_index/storefront_shadow_postgres_tests.rs', [
  'RUSTOK_PRODUCT_STOREFRONT_EQUIVALENCE_DATABASE_URL',
  'SchemaVersion::new(PRODUCT_SCHEMA_ROUTING_KEY)',
  'assert_eq!(owner_c.title, "Untitled product");',
  'assert_eq!(projected_string(index_c, "title")?, None);',
]);
requireMarkers('crates/rustok-distribution/src/product_index/storefront_shadow_eav_postgres_tests.rs', [
  'RUSTOK_PRODUCT_STOREFRONT_EAV_EQUIVALENCE_DATABASE_URL',
  'SchemaVersion::new(PRODUCT_SCHEMA_ROUTING_KEY)',
  'weight=7',
  'label=Punainen',
  'label=Red',
  'color=red',
  'features=wifi',
  'color=missing',
  'color=00000000-0000-0000-0000-000000000000',
]);

const productIndexPath = 'crates/rustok-distribution/src/product_index/product.rs';
const productIndex = requireMarkers(productIndexPath, [
  'assert_eq!(schema.fields.len(), 15);',
  'SchemaVersion::new(PRODUCT_SCHEMA_ROUTING_KEY)',
]);
if (productIndex.includes('SchemaVersion::new(3)')) fail(`${productIndexPath} restored historical key 3`);

requireMarkers('scripts/verify/verify-product-storefront-search-bound.mjs', [
  'MAX_STOREFRONT_PRODUCT_SEARCH_BYTES: usize = 1022',
  'MAX_TEXT_LIKE_PATTERN_BYTES: usize = 1024',
  'ownerBytes + 2 !== indexBytes',
  'reject over-bound input rather than truncate it',
]);
requireMarkers('scripts/verify/verify-index-product-postgres-key4-fixtures.mjs', [
  'product_locale_absence_postgres.rs',
  'product_materialized_query_freshness_postgres.rs',
  'product_channel_convergence_postgres.rs',
  'product_channel_identity_transitions_postgres.rs',
  'product_linked_target_recreate_postgres.rs',
  'product_linked_target_availability_equivalence_postgres.rs',
  'product_linked_target_replay_redelivery_postgres.rs',
  "source.includes('SchemaVersion::new(3)')",
  'PRODUCT_SCHEMA_ROUTING_KEY: u32 = 4',
]);

requireMarkers('crates/rustok-index/docs/m7-product-storefront-parity-gate.md', [
  'Status: `search_bound_source_complete_collation_evidence_pending`',
  'Mounted Storefront remains owner-native',
  'Product-owned Storefront search bound — source complete',
  '`MAX_STOREFRONT_PRODUCT_SEARCH_BYTES = 1022`',
  'Core PostgreSQL packet — source complete, execution pending',
  'EAV PostgreSQL packet — source complete, execution pending',
  'Historical retained Product packets — key 4 source actualized',
  'Owner/default PostgreSQL `pt.title LIKE $1` collation',
  'ProductVariant stays on key',
  'SalesChannel stays on key',
]);
requireMarkers('crates/rustok-index/docs/m7-product-attribute-term-contract.md', [
  'Status: `source_complete_materialized_rebuild_pending`',
  '`requested-value OR (NOT requested-present AND fallback-value)`',
]);
requireMarkers('scripts/verify/verify-index-query-contract.mjs', [
  "'verify-product-storefront-search-bound.mjs'",
  "'verify-index-product-postgres-key4-fixtures.mjs'",
]);

console.log('[verify-index-product-storefront-parity-gate] Storefront remains owner-native; Product search length is source-aligned with TextLike while collation, execution/admission and remaining serving-policy gates stay pending');

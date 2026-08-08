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
for (const forbidden of [
  'rustok_index',
  'SharedIndexQueryRuntime',
  'execute_localized_query',
  'project_product_storefront_index_page',
]) {
  if (storefront.includes(forbidden)) fail(`${storefrontPath} contains forbidden marker ${forbidden}`);
}

requireMarkers('crates/rustok-product/src/services/catalog/types.rs', [
  'pub const MAX_STOREFRONT_PRODUCT_SEARCH_BYTES: usize = 1022;',
  'search: normalize_storefront_product_search(search)?',
  'pub(crate) fn validate_storefront_product_search(',
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
  '.unwrap_or_else(|| "Untitled product".to_string())',
  'handle: translation',
  '.unwrap_or_default()',
  'let pattern = format!("%{search}%");',
  'pt.title LIKE $1',
  'if page == 0 || per_page == 0 || per_page > 48',
  'let offset = (page.saturating_sub(1)) * per_page;',
]);
const titleSearch = owner.slice(owner.indexOf('fn product_title_search_condition('));
if (titleSearch.includes('pt.locale')) fail(`${ownerPath} title search became locale-scoped`);
if (titleSearch.includes('COLLATE')) {
  fail(`${ownerPath} owner title search changed collation before retained default-vs-C evidence admission`);
}
const modernList = owner.slice(
  owner.indexOf('pub async fn list_published_products_with_query('),
  owner.indexOf('pub(crate) async fn list_legacy_storefront_products_with_locale_fallback('),
);
if (modernList.includes('10_000')) {
  fail(`${ownerPath} must not narrow owner-valid Storefront page depth to the Index offset bound`);
}

requireMarkers('crates/rustok-product/src/services/catalog/helpers.rs', [
  'pub(crate) fn product_channel_visibility_condition(',
  "metadata->'channel_visibility'->'allowed_channel_slugs'",
  "jsonb_array_length(COALESCE(products.metadata->'channel_visibility'->'allowed_channel_slugs', '[]'::jsonb)) = 0",
]);
requireMarkers('crates/rustok-distribution/src/product_index/channel_relation_resolver.rs', [
  'ProductChannelVisibility::Unrestricted => (',
  'SELECT id FROM channels WHERE tenant_id = $1 ORDER BY id ASC LIMIT $2',
]);

const executorPath = 'crates/rustok-distribution/src/product_index/storefront_shadow_executor.rs';
const executor = requireMarkers(executorPath, [
  'pub(crate) enum ProductStorefrontIndexChannelScopeDecision',
  'OwnerNativeChannelLess',
  'ChannelLessOwnerNative',
  'pub(crate) enum ProductStorefrontIndexPageScopeDecision',
  'OwnerNativeDeepPage { offset: u64 }',
  'DeepPageOwnerNative { offset: u64 }',
  'pub(crate) projected: Result<IndexQueryPage, ProductStorefrontIndexShadowProjectionError>',
  'pub(crate) public_projected:',
  'Option<Result<IndexQueryPage, ProductStorefrontIndexPublicProjectionError>>',
  'list_filtered_published_products(',
  'classify_product_storefront_index_page_scope(&query)',
  '.resolve_storefront_attribute_filters(',
  '.execute_localized_query(index_query)',
  'let public_projected = projected',
  '.map(project_product_storefront_index_page);',
  'let comparison = projected',
  'compare_owner_and_index(&authoritative, projected)',
  'pub(crate) authoritative: StorefrontProductList',
]);
for (const forbidden of [
  'CHANNEL_LESS_SENTINEL',
  'UNRESTRICTED_CHANNEL_SENTINEL',
  '.min(MAX_INDEX_OFFSET_DEPTH)',
  'Pagination::Cursor',
]) {
  if (executor.includes(forbidden)) fail(`${executorPath} contains forbidden request-shape shortcut ${forbidden}`);
}
const executeProjectedStart = executor.indexOf('async fn execute_projected(');
const compareStart = executor.indexOf('fn compare_owner_and_index(');
if (
  executeProjectedStart < 0 ||
  compareStart <= executeProjectedStart ||
  executor.slice(executeProjectedStart, compareStart).includes('project_product_storefront_index_page')
) {
  fail(`${executorPath} public Product projection must remain outside raw Index execution`);
}

const projectionPath = 'crates/rustok-distribution/src/product_index/storefront_projection.rs';
const projection = requireMarkers(projectionPath, [
  'const UNTITLED_PRODUCT: &str = "Untitled product";',
  'pub(crate) enum ProductStorefrontIndexPublicProjectionError',
  'pub(crate) fn project_product_storefront_index_page(',
  'apply_string_placeholder(item, "title", UNTITLED_PRODUCT)?;',
  'apply_string_placeholder(item, "handle", "")?;',
  'MissingField',
  'DuplicateField',
  'InvalidFieldValue',
  'projected.exact_count, Some(9)',
  'projected.next_cursor.as_deref(), Some("opaque-cursor")',
  'value(&projected.items[0], "tag_ids")',
]);
for (const forbidden of ['FilterExpr', 'OrderExpr', 'LocalizedEntityQuery', 'execute_localized_query']) {
  if (projection.includes(forbidden)) {
    fail(`${projectionPath} must remain post-page only; found ${forbidden}`);
  }
}

const builderPath = 'crates/rustok-distribution/src/product_index/storefront_shadow.rs';
const builder = requireMarkers(builderPath, [
  'services::MAX_STOREFRONT_PRODUCT_SEARCH_BYTES',
  'PublicChannelRequired',
  'root_field("sales_channel_ids")?',
  'const MAX_INDEX_OFFSET_DEPTH: u64 = 10_000;',
  'ProductStorefrontIndexShadowError::OffsetTooDeep',
]);
for (const forbidden of ['Untitled product', 'project_product_storefront_index_page']) {
  if (builder.includes(forbidden)) {
    fail(`${builderPath} must not feed Product public placeholders into query construction: ${forbidden}`);
  }
}

requireMarkers('crates/rustok-distribution/src/product_index/storefront_shadow_postgres_tests.rs', [
  'RUSTOK_PRODUCT_STOREFRONT_EQUIVALENCE_DATABASE_URL',
  'SchemaVersion::new(PRODUCT_SCHEMA_ROUTING_KEY)',
  'assert_eq!(owner_c.title, "Untitled product");',
  'assert_eq!(owner_c.handle, "");',
  'assert_eq!(projected_string(index_c, "title")?, None);',
  'assert_eq!(projected_string(index_c, "handle")?, None);',
]);
requireMarkers('crates/rustok-distribution/src/product_index/storefront_shadow_eav_postgres_tests.rs', [
  'RUSTOK_PRODUCT_STOREFRONT_EAV_EQUIVALENCE_DATABASE_URL',
  'SchemaVersion::new(PRODUCT_SCHEMA_ROUTING_KEY)',
  'weight=7',
  'color=missing',
]);
requireMarkers('crates/rustok-distribution/tests/product_storefront_search_collation_postgres.rs', [
  'RUSTOK_PRODUCT_STOREFRONT_COLLATION_DATABASE_URL',
  'translation.title LIKE $2',
  '(translation.title COLLATE "C") LIKE $2',
  "current_setting('lc_collate')",
]);

const productIndexPath = 'crates/rustok-distribution/src/product_index/product.rs';
const productIndex = requireMarkers(productIndexPath, [
  'assert_eq!(schema.fields.len(), 15);',
  'many_field("tag_ids", IndexValueType::Uuid, true, false)',
  'many_field("sales_channel_ids", IndexValueType::Uuid, true, true)',
  'SchemaVersion::new(PRODUCT_SCHEMA_ROUTING_KEY)',
]);
for (const forbidden of ['SchemaVersion::new(3)', 'SchemaVersion::new(5)']) {
  if (productIndex.includes(forbidden)) fail(`${productIndexPath} contains forbidden Product schema marker ${forbidden}`);
}

requireMarkers('scripts/verify/verify-index-product-storefront-public-projection.mjs', [
  'Product public placeholders are post-page only',
  'raw Index null evidence and tag_ids remain unchanged',
]);
requireMarkers('scripts/verify/verify-index-product-storefront-deep-page-policy.mjs', [
  'OwnerNativeDeepPage { offset: u64 }',
  'must preserve owner pagination without clamp/rewrite',
]);
requireMarkers('scripts/verify/verify-index-product-storefront-channel-scope-policy.mjs', [
  'OwnerNativeChannelLess',
  'must not infer or fabricate visibility membership',
]);
requireMarkers('scripts/verify/verify-index-product-storefront-collation-postgres-packet.mjs', [
  'must remain the owner/default-collation side',
  'must observe deployment/default collation rather than manufacture parity',
]);
requireMarkers('scripts/verify/verify-index-product-postgres-key4-fixtures.mjs', [
  'PRODUCT_SCHEMA_ROUTING_KEY: u32 = 4',
]);

requireMarkers('crates/rustok-index/docs/m7-product-storefront-parity-gate.md', [
  'Status: `public_projection_source_complete_taxonomy_hydration_pending`',
  'Mounted Storefront remains owner-native',
  'Product public placeholder projection — source complete',
  '`public_projected`',
  'Taxonomy tag names',
]);
requireMarkers('crates/rustok-index/docs/m7-product-storefront-public-projection.md', [
  'Status: `source_complete_taxonomy_hydration_pending`',
  '`projected` — raw generic Index page',
  '`public_projected` — optional Product public page',
  '`tag_ids`',
]);
requireMarkers('scripts/verify/verify-index-query-contract.mjs', [
  "'verify-index-product-storefront-public-projection.mjs'",
  "'verify-index-product-storefront-deep-page-policy.mjs'",
  "'verify-index-product-storefront-collation-postgres-packet.mjs'",
]);

console.log('[verify-index-product-storefront-parity-gate] Storefront remains owner-native; raw Index evidence and Product post-page public placeholders are separated while Taxonomy hydration, evidence admission and serving gates remain pending');

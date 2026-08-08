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
  'hydrate_storefront_product_tags',
  'ProductStorefrontIndexServingBudget',
  'classify_product_storefront_index_serving_budget',
]) {
  if (storefront.includes(forbidden)) fail(`${storefrontPath} contains forbidden marker ${forbidden}`);
}

requireMarkers('crates/rustok-product/src/services/catalog/types.rs', [
  'pub const MAX_STOREFRONT_PRODUCT_SEARCH_BYTES: usize = 1022;',
  'search: normalize_storefront_product_search(search)?',
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

const tagPortPath = 'crates/rustok-product/src/storefront_tag_read_port.rs';
const tagPort = requireMarkers(tagPortPath, [
  'const MAX_STOREFRONT_TAG_HYDRATION_PRODUCTS: usize = 48;',
  'pub trait ProductStorefrontTagReadPort',
  'impl ProductStorefrontTagReadPort for CatalogService',
  '.load_product_tag_map(',
  'context.locale.as_str()',
  'Some(request.fallback_locale.as_str())',
]);
for (const forbidden of ['rustok_index', 'IndexQueryPage', 'IndexValue']) {
  if (tagPort.includes(forbidden)) fail(`${tagPortPath} must remain Product-owned and Index-neutral: ${forbidden}`);
}
requireMarkers('crates/rustok-product/src/services/catalog/tags.rs', [
  'TaxonomyService::new(self.db.clone())',
  '.resolve_term_names(tenant_id, &ordered_term_ids, locale, fallback_locale)',
  'metadata_has_tags_field(&product.metadata)',
  'normalize_tag_names(&extract_metadata_tags(&product.metadata))',
]);

const executorPath = 'crates/rustok-distribution/src/product_index/storefront_shadow_executor.rs';
const executor = requireMarkers(executorPath, [
  'OwnerNativeChannelLess',
  'ChannelLessOwnerNative',
  'OwnerNativeDeepPage { offset: u64 }',
  'DeepPageOwnerNative { offset: u64 }',
  'pub(crate) projected: Result<IndexQueryPage, ProductStorefrontIndexShadowProjectionError>',
  'pub(crate) public_projected:',
  'pub(crate) tag_hydration:',
  'TagReadPortUnavailable',
  'list_filtered_published_products(',
  '.execute_localized_query(index_query)',
  'let public_projected = projected',
  '.map(project_product_storefront_index_page);',
  'let tag_hydration = match projected.as_ref()',
  'self.hydrate_projected_tags(context, fallback_locale, projected)',
  '.storefront_tag_read_port()',
  '.map(|item| item.entity_id)',
  '.hydrate_storefront_product_tags(',
  'let comparison = projected',
  'compare_owner_and_index(&authoritative, projected)',
]);
for (const forbidden of [
  'CHANNEL_LESS_SENTINEL',
  'UNRESTRICTED_CHANNEL_SENTINEL',
  '.min(MAX_INDEX_OFFSET_DEPTH)',
  'Pagination::Cursor',
  'TaxonomyService',
  'product_tag::',
  'DatabaseConnection',
  'classify_product_storefront_index_serving_budget',
]) {
  if (executor.includes(forbidden)) fail(`${executorPath} contains forbidden shortcut/storage/serving marker ${forbidden}`);
}

const policyPath = 'crates/rustok-distribution/src/product_index/storefront_serving_budget.rs';
const policy = requireMarkers(policyPath, [
  'pub(crate) struct ProductStorefrontIndexServingBudget',
  'index_execution_ms: u64',
  'tag_hydration_ms: u64',
  'safety_margin_ms: u64',
  'required_ms: u64',
  'checked_add(tag_hydration_ms)',
  'checked_add(safety_margin_ms)',
  'pub(crate) struct ProductStorefrontIndexServingBudgetObservation',
  'pub(crate) remaining_ms: Option<u64>',
  'pub(crate) tag_hydration_available: bool',
  'pub(crate) enum ProductStorefrontIndexServingBudgetDecision',
  'OwnerNativeMissingDeadline',
  'OwnerNativeBudgetPolicyUnavailable',
  'OwnerNativeRemainingBudgetUnavailable',
  'OwnerNativeInvalidRemainingBudget',
  'OwnerNativeTagHydrationUnavailable',
  'OwnerNativeInsufficientBudget',
  'pub(crate) fn classify_product_storefront_index_serving_budget(',
  'context.deadline_ms.filter(|deadline_ms| *deadline_ms > 0)',
  'if remaining_ms > deadline_ms',
  'if !observation.tag_hydration_available',
  'if remaining_ms < budget.required_ms()',
  'ProductStorefrontIndexServingBudgetDecision::Eligible',
  'does not automatically decrease',
]);
const productionPolicy = policy.split('#[cfg(test)]')[0];
for (const forbidden of ['Instant::now()', 'SystemTime::now()', 'tokio::time::timeout']) {
  if (productionPolicy.includes(forbidden)) {
    fail(`${policyPath} policy must classify host observations without manufacturing/enforcing timing: ${forbidden}`);
  }
}

const projectionPath = 'crates/rustok-distribution/src/product_index/storefront_projection.rs';
const projection = requireMarkers(projectionPath, [
  'const UNTITLED_PRODUCT: &str = "Untitled product";',
  'pub(crate) fn project_product_storefront_index_page(',
  'value(&projected.items[0], "tag_ids")',
]);
for (const forbidden of ['FilterExpr', 'OrderExpr', 'LocalizedEntityQuery', 'execute_localized_query']) {
  if (projection.includes(forbidden)) fail(`${projectionPath} must remain post-page only; found ${forbidden}`);
}

const productIndexPath = 'crates/rustok-distribution/src/product_index/product.rs';
const productIndex = requireMarkers(productIndexPath, [
  'assert_eq!(schema.fields.len(), 15);',
  'many_field("tag_ids", IndexValueType::Uuid, true, false)',
  'many_field("sales_channel_ids", IndexValueType::Uuid, true, true)',
  'SchemaVersion::new(PRODUCT_SCHEMA_ROUTING_KEY)',
]);
for (const forbidden of ['SchemaVersion::new(3)', 'SchemaVersion::new(5)', 'tag_names', 'localized_tag_names']) {
  if (productIndex.includes(forbidden)) fail(`${productIndexPath} contains forbidden Product schema/source marker ${forbidden}`);
}

requireMarkers('crates/rustok-distribution/src/product_index/storefront_shadow_postgres_tests.rs', [
  'RUSTOK_PRODUCT_STOREFRONT_EQUIVALENCE_DATABASE_URL',
  'SchemaVersion::new(PRODUCT_SCHEMA_ROUTING_KEY)',
  'assert_eq!(owner_c.title, "Untitled product");',
  'assert_eq!(projected_string(index_c, "title")?, None);',
]);
requireMarkers('crates/rustok-distribution/tests/product_storefront_search_collation_postgres.rs', [
  'RUSTOK_PRODUCT_STOREFRONT_COLLATION_DATABASE_URL',
  'translation.title LIKE $2',
  '(translation.title COLLATE "C") LIKE $2',
]);

requireMarkers('scripts/verify/verify-index-product-storefront-public-projection.mjs', [
  'Product public placeholders are post-page only',
]);
requireMarkers('scripts/verify/verify-index-product-storefront-tag-hydration.mjs', [
  'Product IDs from the fixed raw Index page drive bounded Product-owned tag hydration',
]);
requireMarkers('scripts/verify/verify-index-product-storefront-serving-budget-policy.mjs', [
  'future serving handoff requires explicit host-measured remaining budget',
  'mounted Storefront remains owner-native',
]);
requireMarkers('scripts/verify/verify-index-product-storefront-deep-page-policy.mjs', [
  'OwnerNativeDeepPage { offset: u64 }',
]);
requireMarkers('scripts/verify/verify-index-product-storefront-channel-scope-policy.mjs', [
  'OwnerNativeChannelLess',
]);
requireMarkers('scripts/verify/verify-index-product-storefront-collation-postgres-packet.mjs', [
  'must observe deployment/default collation rather than manufacture parity',
]);
requireMarkers('scripts/verify/verify-index-product-postgres-key4-fixtures.mjs', [
  'PRODUCT_SCHEMA_ROUTING_KEY: u32 = 4',
]);

requireMarkers('crates/rustok-index/docs/m7-product-storefront-parity-gate.md', [
  'Status: `serving_budget_policy_source_complete_timeout_enforcement_pending`',
  'Mounted Storefront remains owner-native',
  'Post-owner serving-budget policy — source complete',
  'host-measured `remaining_ms`',
  'classification policy',
  'runtime timeout enforcement',
]);
requireMarkers('crates/rustok-index/docs/m7-product-storefront-serving-budget-policy.md', [
  'Status: `policy_source_complete_timeout_enforcement_pending`',
  '`PortContext.deadline_ms` is not enough',
  'host-measured `remaining_ms`',
  'non-serving budgeted execution adapter',
]);
requireMarkers('scripts/verify/verify-index-query-contract.mjs', [
  "'verify-index-product-storefront-tag-hydration.mjs'",
  "'verify-index-product-storefront-serving-budget-policy.mjs'",
  "'verify-index-product-storefront-collation-postgres-packet.mjs'",
]);

console.log('[verify-index-product-storefront-parity-gate] Storefront remains owner-native; request shape, post-page owner projection and host-measured serving-budget policy are source-complete while timeout enforcement and evidence admission remain pending');

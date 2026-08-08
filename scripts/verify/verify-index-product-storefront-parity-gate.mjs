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

const mountedPath = 'crates/rustok-product/storefront/src/transport/catalog_list_native.rs';
const mounted = requireMarkers(mountedPath, [
  'CatalogService::new(runtime_ctx.db_clone(), event_bus)',
  '.list_published_products_with_query(',
]);
for (const forbidden of [
  'rustok_index',
  'ProductStorefrontIndexShadowExecutor',
  'ProductStorefrontIndexBudgetedProjectionExecutor',
  'ProductStorefrontIndexServingBudget',
  'classify_product_storefront_index_serving_budget',
  'execute_localized_query',
]) {
  if (mounted.includes(forbidden)) fail(`${mountedPath} must remain owner-native; found ${forbidden}`);
}

requireMarkers('crates/rustok-product/src/services/catalog/types.rs', [
  'pub const MAX_STOREFRONT_PRODUCT_SEARCH_BYTES: usize = 1022;',
]);
requireMarkers('crates/rustok-product/src/services/catalog/queries.rs', [
  'product_channel_visibility_condition(',
  'attribute_filters::load_catalog_attribute_filter_conditions(',
  'types::validate_storefront_product_search(list_query.search.as_deref())?;',
  'let total = query.clone().count(&self.db).await?',
  '.unwrap_or_else(|| "Untitled product".to_string())',
  'let pattern = format!("%{search}%");',
  'pt.title LIKE $1',
  'let offset = (page.saturating_sub(1)) * per_page;',
]);
requireMarkers('crates/rustok-product/src/storefront_tag_read_port.rs', [
  'MAX_STOREFRONT_TAG_HYDRATION_PRODUCTS: usize = 48',
  'pub trait ProductStorefrontTagReadPort',
  '.load_product_tag_map(',
]);

const shadowPath = 'crates/rustok-distribution/src/product_index/storefront_shadow_executor.rs';
const shadow = requireMarkers(shadowPath, [
  'OwnerNativeChannelLess',
  'OwnerNativeDeepPage { offset: u64 }',
  'pub(crate) async fn execute_projected(',
  'pub(crate) async fn hydrate_projected_tags(',
  'list_filtered_published_products(',
  '.execute_localized_query(index_query)',
  '.hydrate_storefront_product_tags(',
]);
for (const forbidden of ['tokio::time::timeout', 'ProductStorefrontIndexServingBudgetDecision']) {
  if (shadow.includes(forbidden)) fail(`${shadowPath} evidence executor must stay unbudgeted: ${forbidden}`);
}

const policyPath = 'crates/rustok-distribution/src/product_index/storefront_serving_budget.rs';
const policy = requireMarkers(policyPath, [
  'ProductStorefrontIndexServingBudget',
  'ProductStorefrontIndexServingBudgetObservation',
  'pub(crate) remaining_ms: Option<u64>',
  'OwnerNativeMissingDeadline',
  'OwnerNativeBudgetPolicyUnavailable',
  'OwnerNativeRemainingBudgetUnavailable',
  'OwnerNativeTagHydrationUnavailable',
  'OwnerNativeInsufficientBudget',
  'ProductStorefrontIndexServingBudgetDecision::Eligible',
  'does not automatically decrease',
]);
if (policy.split('#[cfg(test)]')[0].includes('tokio::time::timeout')) {
  fail(`${policyPath} classification policy must remain separate from timeout enforcement`);
}

const budgetedPath = 'crates/rustok-distribution/src/product_index/storefront_budgeted_execution.rs';
const budgeted = requireMarkers(budgetedPath, [
  'use tokio::time::timeout;',
  'pub(crate) trait ProductStorefrontIndexProjectionPhases',
  'impl ProductStorefrontIndexProjectionPhases for ProductStorefrontIndexShadowExecutor',
  'phases: Arc<dyn ProductStorefrontIndexProjectionPhases>',
  'pub(crate) struct ProductStorefrontIndexBudgetedExecution',
  'pub(crate) authoritative: StorefrontProductList',
  'pub(crate) struct ProductStorefrontIndexBudgetedProjectionExecutor',
  'pub(crate) async fn execute_after_owner(',
  'ProductStorefrontIndexServingBudgetDecision::Eligible',
  'BudgetNotEligible',
  'index_context.deadline_ms = Some(index_execution_budget_ms);',
  'Duration::from_millis(index_execution_budget_ms)',
  'self.phases.execute_projected(',
  'ProductStorefrontIndexBudgetedProjectionError::TimedOut',
  '.map(project_product_storefront_index_page);',
  'tag_context.deadline_ms = Some(tag_hydration_budget_ms);',
  'Duration::from_millis(tag_hydration_budget_ms)',
  '.hydrate_projected_tags(tag_context, fallback_locale, projected)',
  'ProductStorefrontIndexBudgetedTagHydrationError::TimedOut',
  'compare_owner_and_projected(&authoritative, projected)',
]);
if (budgeted.includes('list_filtered_published_products(')) {
  fail(`${budgetedPath} must be post-owner and must not repeat the authoritative Product read`);
}

const productIndexPath = 'crates/rustok-distribution/src/product_index/product.rs';
const productIndex = requireMarkers(productIndexPath, [
  'derive_index_schema_source_event_id',
  'assert_eq!(schema.fields.len(), 15);',
  'many_field("tag_ids", IndexValueType::Uuid, true, false)',
  'many_field("sales_channel_ids", IndexValueType::Uuid, true, true)',
  'SchemaVersion::new(PRODUCT_SCHEMA_ROUTING_KEY)',
]);
for (const forbidden of ['SchemaVersion::new(3)', 'SchemaVersion::new(5)', 'localized_tag_names', 'derive_index_source_event_id(']) {
  if (productIndex.includes(forbidden)) fail(`${productIndexPath} contains forbidden Product schema marker ${forbidden}`);
}

requireMarkers('scripts/verify/verify-index-product-current-schema-promotion.mjs', [
  'Product key4 is the only current runtime contract',
  'tenant stage/rebuild/register_current promotion remains fail-closed and execution-owned',
]);
requireMarkers('crates/rustok-index/docs/m7-product-current-schema-promotion.md', [
  'Status: `source_contract_complete_execution_pending`',
  'Tenant promotion sequence',
  '`PostgresSchemaRegistrationStore::register_current`',
  'Mounted Storefront remains owner-native',
]);

requireMarkers('crates/rustok-distribution/src/product_index/storefront_projection.rs', [
  'const UNTITLED_PRODUCT: &str = "Untitled product";',
  'pub(crate) fn project_product_storefront_index_page(',
  'value(&projected.items[0], "tag_ids")',
]);
requireMarkers('crates/rustok-distribution/src/product_index/storefront_shadow_postgres_tests.rs', [
  'SchemaVersion::new(PRODUCT_SCHEMA_ROUTING_KEY)',
  'assert_eq!(owner_c.title, "Untitled product");',
  'assert_eq!(projected_string(index_c, "title")?, None);',
]);
requireMarkers('crates/rustok-distribution/tests/product_storefront_search_collation_postgres.rs', [
  'translation.title LIKE $2',
  '(translation.title COLLATE "C") LIKE $2',
]);

requireMarkers('scripts/verify/verify-index-product-storefront-serving-budget-policy.mjs', [
  'production phase seam',
]);
requireMarkers('scripts/verify/verify-index-product-storefront-budgeted-execution.mjs', [
  'production phase seam',
]);
requireMarkers('scripts/verify/verify-index-product-storefront-budgeted-execution-evidence.mjs', [
  'retained fake-phase packet covers noneligible, Index timeout/error, tag timeout, phase deadlines and fast-path identity/count',
]);
requireMarkers('scripts/verify/verify-index-product-storefront-shadow-executor.mjs', [
  'implements the crate-private post-owner phase seam',
]);
requireMarkers('scripts/verify/verify-index-product-storefront-tag-hydration.mjs', [
  'Product IDs from the fixed raw Index page drive bounded Product-owned tag hydration',
]);
requireMarkers('scripts/verify/verify-index-product-postgres-key4-fixtures.mjs', [
  'PRODUCT_SCHEMA_ROUTING_KEY: u32 = 4',
]);

requireMarkers('crates/rustok-index/docs/m7-product-storefront-parity-gate.md', [
  'Status: `budgeted_timeout_evidence_source_complete_execution_pending`',
  'Mounted Storefront remains owner-native',
  'Serving-budget policy and timeout enforcement — source complete',
  'Deterministic timeout evidence — source complete, execution pending',
  'Current Product key-4 promotion — source contract complete, execution pending',
  '`ProductStorefrontIndexBudgetedProjectionExecutor`',
  'The packet has not been executed by the implementation agent',
]);
requireMarkers('crates/rustok-index/docs/m7-product-storefront-serving-budget-policy.md', [
  'Status: `policy_and_timeout_enforcement_source_complete_runtime_evidence_pending`',
  '`tokio::time::timeout`',
]);
requireMarkers('crates/rustok-index/docs/m7-product-storefront-budgeted-execution.md', [
  'Status: `source_complete_timeout_evidence_execution_pending`',
  'Retained deterministic timeout evidence — source complete',
  'The retained packet is **source-only** until a maintainer executes it.',
]);
requireMarkers('scripts/verify/verify-index-query-contract.mjs', [
  "'verify-index-product-current-schema-promotion.mjs'",
  "'verify-index-product-storefront-serving-budget-policy.mjs'",
  "'verify-index-product-storefront-budgeted-execution.mjs'",
  "'verify-index-product-storefront-budgeted-execution-evidence.mjs'",
]);

console.log('[verify-index-product-storefront-parity-gate] Storefront remains owner-native; current key4 promotion, budget policy, timeout enforcement and deterministic timeout evidence are source-complete while maintainer execution/admission remains pending');

#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const read = (relative) => fs.readFileSync(path.join(root, relative), 'utf8');
const fail = (message) => {
  console.error(`[verify-index-product-storefront-budgeted-execution] ${message}`);
  process.exit(1);
};
const requireMarkers = (relative, markers) => {
  const source = read(relative);
  for (const marker of markers) {
    if (!source.includes(marker)) fail(`${relative} is missing ${marker}`);
  }
  return source;
};

const cargoPath = 'crates/rustok-distribution/Cargo.toml';
const cargo = requireMarkers(cargoPath, [
  '[dependencies]',
  'tokio.workspace = true',
]);
const devSection = cargo.split('[dev-dependencies]')[1] ?? '';
if (devSection.includes('tokio.workspace = true')) {
  fail(`${cargoPath} must not retain a duplicate dev-only Tokio declaration once timeout enforcement is production source`);
}

const budgetPath = 'crates/rustok-distribution/src/product_index/storefront_serving_budget.rs';
requireMarkers(budgetPath, [
  'ProductStorefrontIndexServingBudgetDecision::Eligible',
  'index_execution_ms: u64',
  'tag_hydration_ms: u64',
  'safety_margin_ms: u64',
]);

const executionPath = 'crates/rustok-distribution/src/product_index/storefront_budgeted_execution.rs';
const execution = requireMarkers(executionPath, [
  'use tokio::time::timeout;',
  'pub(crate) struct ProductStorefrontIndexBudgetedExecution',
  'pub(crate) authoritative: StorefrontProductList',
  'pub(crate) projected: Result<IndexQueryPage, ProductStorefrontIndexBudgetedProjectionError>',
  'pub(crate) public_projected:',
  'pub(crate) tag_hydration:',
  'pub(crate) comparison: Option<ProductStorefrontIndexShadowComparison>',
  'pub(crate) index_execution_budget_ms: u64',
  'pub(crate) tag_hydration_budget_ms: u64',
  'pub(crate) safety_margin_ms: u64',
  'BudgetNotEligible(ProductStorefrontIndexServingBudgetDecision)',
  'ProductStorefrontIndexBudgetedProjectionError',
  'TimedOut { budget_ms: u64 }',
  'ProductStorefrontIndexBudgetedTagHydrationError',
  'pub(crate) struct ProductStorefrontIndexBudgetedProjectionExecutor',
  'pub(crate) async fn execute_after_owner(',
  'authoritative: StorefrontProductList',
  'ProductStorefrontIndexServingBudgetDecision::Eligible',
  'other => return Err(ProductStorefrontIndexBudgetedStartError::BudgetNotEligible(other))',
  'index_context.deadline_ms = Some(index_execution_budget_ms);',
  'timeout(',
  'Duration::from_millis(index_execution_budget_ms)',
  'self.shadow.execute_projected(',
  'ProductStorefrontIndexBudgetedProjectionError::TimedOut',
  '.map(project_product_storefront_index_page);',
  'tag_context.deadline_ms = Some(tag_hydration_budget_ms);',
  'Duration::from_millis(tag_hydration_budget_ms)',
  '.hydrate_projected_tags(tag_context, fallback_locale, projected)',
  'ProductStorefrontIndexBudgetedTagHydrationError::TimedOut',
  'compare_owner_and_projected(&authoritative, projected)',
]);

for (const forbidden of [
  'list_filtered_published_products(',
  'ProductCatalogReadRuntime',
  'CatalogService::new',
  'DatabaseConnection',
  'TaxonomyService',
  'query_all(',
  'query_one(',
]) {
  if (execution.includes(forbidden)) {
    fail(`${executionPath} must remain a post-owner adapter over selected shadow capabilities; found ${forbidden}`);
  }
}

const matchPosition = execution.indexOf('let (index_execution_budget_ms, tag_hydration_budget_ms, safety_margin_ms) = match decision');
const firstTimeoutPosition = execution.indexOf('let projected = match timeout(');
if (matchPosition < 0 || firstTimeoutPosition <= matchPosition) {
  fail('budget eligibility must be resolved before any timeout/execution starts');
}
const preTimeout = execution.slice(matchPosition, firstTimeoutPosition);
if (!preTimeout.includes('BudgetNotEligible')) {
  fail('non-eligible decisions must fail closed before projected execution');
}

const projectedPosition = execution.indexOf('let projected = match timeout(');
const publicPosition = execution.indexOf('let public_projected = projected');
const tagPosition = execution.indexOf('let tag_hydration = match projected.as_ref()');
const comparisonPosition = execution.indexOf('let comparison = projected');
if (
  projectedPosition < 0 ||
  publicPosition <= projectedPosition ||
  tagPosition <= projectedPosition ||
  comparisonPosition <= projectedPosition
) {
  fail('public projection, tag hydration and comparison must follow the bounded raw Index phase');
}

const executorPath = 'crates/rustok-distribution/src/product_index/storefront_shadow_executor.rs';
const executor = requireMarkers(executorPath, [
  'pub(crate) async fn execute_projected(',
  'pub(crate) async fn hydrate_projected_tags(',
  'pub(crate) async fn execute(',
]);
if (executor.includes('tokio::time::timeout') || executor.includes('ProductStorefrontIndexServingBudgetDecision')) {
  fail(`${executorPath} evidence executor must stay separate from serving-budget timeout enforcement`);
}

requireMarkers('crates/rustok-distribution/src/product_index/mod.rs', [
  'mod storefront_budgeted_execution;',
  'ProductStorefrontIndexBudgetedExecution',
  'ProductStorefrontIndexBudgetedProjectionExecutor',
  'ProductStorefrontIndexBudgetedStartError',
  'ProductStorefrontIndexBudgetedProjectionError',
  'ProductStorefrontIndexBudgetedTagHydrationError',
]);

const mountedPath = 'crates/rustok-product/storefront/src/transport/catalog_list_native.rs';
const mounted = read(mountedPath);
for (const forbidden of [
  'ProductStorefrontIndexBudgetedProjectionExecutor',
  'ProductStorefrontIndexServingBudget',
  'classify_product_storefront_index_serving_budget',
  'execute_localized_query',
]) {
  if (mounted.includes(forbidden)) {
    fail(`${mountedPath} must remain owner-native; found budgeted serving marker ${forbidden}`);
  }
}

console.log('[verify-index-product-storefront-budgeted-execution] eligible post-owner projection applies bounded Index/tag timeouts while preserving the authoritative owner result; mounted Storefront remains owner-native');

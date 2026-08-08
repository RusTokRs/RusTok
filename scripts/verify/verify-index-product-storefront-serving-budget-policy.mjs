#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const read = (relative) => fs.readFileSync(path.join(root, relative), 'utf8');
const fail = (message) => {
  console.error(`[verify-index-product-storefront-serving-budget-policy] ${message}`);
  process.exit(1);
};
const requireMarkers = (relative, markers) => {
  const source = read(relative);
  for (const marker of markers) {
    if (!source.includes(marker)) fail(`${relative} is missing ${marker}`);
  }
  return source;
};

requireMarkers('crates/rustok-api/src/ports.rs', [
  'pub deadline_ms: Option<u64>',
  'pub fn with_deadline(mut self, deadline: Duration)',
  'self.deadline_ms = Some(deadline.as_millis()',
  'pub fn require_deadline_semantics(&self)',
]);

const policyPath = 'crates/rustok-distribution/src/product_index/storefront_serving_budget.rs';
const policy = requireMarkers(policyPath, [
  'pub(crate) struct ProductStorefrontIndexServingBudget',
  'index_execution_ms: u64',
  'tag_hydration_ms: u64',
  'safety_margin_ms: u64',
  'required_ms: u64',
  'checked_add(tag_hydration_ms)',
  'checked_add(safety_margin_ms)',
  'ZeroIndexExecutionBudget',
  'ZeroTagHydrationBudget',
  'BudgetOverflow',
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
for (const forbidden of [
  'remaining_ms: context.deadline_ms',
  'remaining_ms = context.deadline_ms',
  'unwrap_or(context.deadline_ms',
  'Instant::now()',
  'SystemTime::now()',
  'tokio::time::timeout',
]) {
  if (productionPolicy.includes(forbidden)) {
    fail(`${policyPath} must classify host-measured remaining budget, not manufacture or enforce timing: ${forbidden}`);
  }
}

const budgetedPath = 'crates/rustok-distribution/src/product_index/storefront_budgeted_execution.rs';
const budgeted = requireMarkers(budgetedPath, [
  'use tokio::time::timeout;',
  'pub(crate) struct ProductStorefrontIndexBudgetedProjectionExecutor',
  'pub(crate) async fn execute_after_owner(',
  'ProductStorefrontIndexServingBudgetDecision::Eligible',
  'BudgetNotEligible',
  'index_context.deadline_ms = Some(index_execution_budget_ms);',
  'Duration::from_millis(index_execution_budget_ms)',
  'self.shadow.execute_projected(',
  'tag_context.deadline_ms = Some(tag_hydration_budget_ms);',
  'Duration::from_millis(tag_hydration_budget_ms)',
  '.hydrate_projected_tags(tag_context, fallback_locale, projected)',
  'ProductStorefrontIndexBudgetedProjectionError::TimedOut',
  'ProductStorefrontIndexBudgetedTagHydrationError::TimedOut',
  'pub(crate) authoritative: StorefrontProductList',
]);
if (budgeted.includes('list_filtered_published_products(')) {
  fail(`${budgetedPath} is post-owner only and must not repeat the authoritative owner read`);
}

const executorPath = 'crates/rustok-distribution/src/product_index/storefront_shadow_executor.rs';
const executor = requireMarkers(executorPath, [
  'pub(crate) async fn execute_projected(',
  'pub(crate) async fn hydrate_projected_tags(',
  'pub(crate) async fn execute(',
]);
for (const forbidden of ['tokio::time::timeout', 'ProductStorefrontIndexServingBudgetDecision']) {
  if (executor.includes(forbidden)) {
    fail(`${executorPath} evidence executor must remain separate from serving-budget enforcement: ${forbidden}`);
  }
}

requireMarkers('crates/rustok-product/src/runtime.rs', [
  'storefront_tag_read_port: Option<Arc<dyn ProductStorefrontTagReadPort>>',
  'pub fn storefront_tag_read_port(&self)',
]);
requireMarkers('crates/rustok-distribution/src/product_index/mod.rs', [
  'mod storefront_serving_budget;',
  'mod storefront_budgeted_execution;',
  'ProductStorefrontIndexServingBudget',
  'ProductStorefrontIndexServingBudgetDecision',
  'ProductStorefrontIndexServingBudgetObservation',
  'classify_product_storefront_index_serving_budget',
  'ProductStorefrontIndexBudgetedProjectionExecutor',
]);

const mountedPath = 'crates/rustok-product/storefront/src/transport/catalog_list_native.rs';
const mounted = read(mountedPath);
for (const forbidden of [
  'ProductStorefrontIndexServingBudget',
  'classify_product_storefront_index_serving_budget',
  'ProductStorefrontIndexBudgetedProjectionExecutor',
  'ProductStorefrontIndexShadowExecutor',
  'execute_localized_query',
]) {
  if (mounted.includes(forbidden)) {
    fail(`${mountedPath} must remain owner-native; found serving-budget/Index marker ${forbidden}`);
  }
}

console.log('[verify-index-product-storefront-serving-budget-policy] host-measured eligibility stays pure, separate budgeted execution enforces admitted phase timeouts, and mounted Storefront remains owner-native');

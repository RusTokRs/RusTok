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

const portContextPath = 'crates/rustok-api/src/ports.rs';
requireMarkers(portContextPath, [
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
  'pub(crate) fn new(',
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
  'let Some(budget) = budget else',
  'let Some(remaining_ms) = observation.remaining_ms else',
  'if remaining_ms > deadline_ms',
  'if !observation.tag_hydration_available',
  'if remaining_ms < budget.required_ms()',
  'ProductStorefrontIndexServingBudgetDecision::Eligible',
  'requires_host_measured_remaining_budget_and_owner_tag_capability',
  'admits_only_when_remaining_budget_covers_all_bounded_phases',
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
    fail(`${policyPath} must consume host-measured remaining budget rather than manufacture/enforce it here: ${forbidden}`);
  }
}
if (!productionPolicy.includes('does not automatically decrease')) {
  fail(`${policyPath} must document that PortContext.deadline_ms is not remaining time`);
}

const productRuntimePath = 'crates/rustok-product/src/runtime.rs';
requireMarkers(productRuntimePath, [
  'storefront_tag_read_port: Option<Arc<dyn ProductStorefrontTagReadPort>>',
  'pub fn storefront_tag_read_port(&self)',
]);

requireMarkers('crates/rustok-distribution/src/product_index/mod.rs', [
  'mod storefront_serving_budget;',
  'ProductStorefrontIndexServingBudget',
  'ProductStorefrontIndexServingBudgetDecision',
  'ProductStorefrontIndexServingBudgetObservation',
  'classify_product_storefront_index_serving_budget',
]);

const mountedPath = 'crates/rustok-product/storefront/src/transport/catalog_list_native.rs';
const mounted = read(mountedPath);
for (const forbidden of [
  'ProductStorefrontIndexServingBudget',
  'classify_product_storefront_index_serving_budget',
  'ProductStorefrontIndexShadowExecutor',
  'execute_localized_query',
]) {
  if (mounted.includes(forbidden)) {
    fail(`${mountedPath} must remain owner-native and must not consume serving-budget/Index policy yet: ${forbidden}`);
  }
}

const executorPath = 'crates/rustok-distribution/src/product_index/storefront_shadow_executor.rs';
const executor = requireMarkers(executorPath, [
  'pub(crate) tag_hydration:',
  'TagReadPortUnavailable',
  'let tag_hydration = match projected.as_ref()',
]);
if (executor.includes('classify_product_storefront_index_serving_budget')) {
  fail(`${executorPath} current evidence executor must not be silently promoted into a serving router`);
}

console.log('[verify-index-product-storefront-serving-budget-policy] future serving handoff requires explicit host-measured remaining budget, bounded Index/tag phases, and tag capability; mounted Storefront remains owner-native');

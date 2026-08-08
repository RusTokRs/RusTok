#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const read = (relative) => fs.readFileSync(path.join(root, relative), 'utf8');
const fail = (message) => {
  console.error(`[verify-index-product-storefront-budgeted-execution-evidence] ${message}`);
  process.exit(1);
};
const requireMarkers = (relative, markers) => {
  const source = read(relative);
  for (const marker of markers) {
    if (!source.includes(marker)) fail(`${relative} is missing ${marker}`);
  }
  return source;
};

const executionPath = 'crates/rustok-distribution/src/product_index/storefront_budgeted_execution.rs';
const execution = requireMarkers(executionPath, [
  'pub(crate) trait ProductStorefrontIndexProjectionPhases',
  'impl ProductStorefrontIndexProjectionPhases for ProductStorefrontIndexShadowExecutor',
  'phases: Arc<dyn ProductStorefrontIndexProjectionPhases>',
  'pub(crate) fn new(shadow: ProductStorefrontIndexShadowExecutor)',
  'pub(crate) fn from_phases(phases: Arc<dyn ProductStorefrontIndexProjectionPhases>)',
  'ProductStorefrontIndexServingBudgetDecision::Eligible',
  'BudgetNotEligible',
  'use tokio::time::timeout;',
  'self.phases.execute_projected(',
  'self.phases',
  '.hydrate_projected_tags(tag_context, fallback_locale, projected)',
]);
if (execution.includes('list_filtered_published_products(')) {
  fail(`${executionPath} retained budget evidence seam must remain post-owner only`);
}

const packetPath = 'crates/rustok-distribution/src/product_index/storefront_budgeted_execution_tests.rs';
const packet = requireMarkers(packetPath, [
  'future::pending',
  'AtomicUsize',
  'index_deadlines: Mutex<Vec<Option<u64>>>',
  'tag_deadlines: Mutex<Vec<Option<u64>>>',
  'impl ProductStorefrontIndexProjectionPhases for FakePhases',
  'pending::<()>().await;',
  'noneligible_budget_starts_no_projected_work',
  'state.index_calls.load(Ordering::SeqCst), 0',
  'state.tag_calls.load(Ordering::SeqCst), 0',
  'index_timeout_preserves_authoritative_owner_page_and_skips_enrichment',
  'ProductStorefrontIndexBudgetedProjectionError::TimedOut { budget_ms: 1 }',
  'execution.public_projected.is_none()',
  'execution.tag_hydration.is_none()',
  'raw_projection_failure_skips_public_projection_and_tag_hydration',
  'ProductStorefrontIndexShadowProjectionError::InvalidTenant',
  'tag_timeout_preserves_raw_public_pages_and_phase_deadlines',
  'projected_string(public, "title"), Some("Untitled product")',
  'ProductStorefrontIndexBudgetedTagHydrationError::TimedOut { budget_ms: 1 }',
  'vec![Some(40)]',
  'vec![Some(1)]',
  'eligible_fast_path_preserves_identity_count_and_owner_tag_projection',
  'raw.exact_count, Some(1)',
  'execution.comparison.unwrap().is_match()',
  'execution.index_execution_budget_ms, 50',
  'execution.tag_hydration_budget_ms, 30',
  'hydrated.items[0].tags, vec!["Legacy".to_owned()]',
  'vec![Some(50)]',
  'vec![Some(30)]',
]);
for (const forbidden of [
  'DatabaseConnection',
  'Postgres',
  'CatalogService::new',
  'SharedIndexQueryRuntime',
  'materialize_postgres',
]) {
  if (packet.includes(forbidden)) {
    fail(`${packetPath} must remain deterministic fake-phase timeout evidence without external storage/runtime setup: ${forbidden}`);
  }
}

requireMarkers('crates/rustok-distribution/src/product_index/mod.rs', [
  'ProductStorefrontIndexProjectionPhases',
  '#[cfg(test)]\nmod storefront_budgeted_execution_tests;',
]);

console.log('[verify-index-product-storefront-budgeted-execution-evidence] retained fake-phase packet covers noneligible, Index timeout/error, tag timeout, phase deadlines and fast-path identity/count; execution remains maintainer-owned');

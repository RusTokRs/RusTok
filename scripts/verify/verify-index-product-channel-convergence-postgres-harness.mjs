#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const read = (relative) => fs.readFileSync(path.join(root, relative), 'utf8');
const fail = (message) => {
  console.error(`[verify-index-product-channel-convergence-postgres-harness] ${message}`);
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

const harnessPath = 'crates/rustok-distribution/tests/product_channel_convergence_postgres.rs';
const harness = requireMarkers(harnessPath, [
  '#![cfg(feature = "mod-product")]',
  'RUSTOK_INDEX_TEST_DATABASE_URL',
  'rustok_channel::migrations::migrations()',
  'rustok_product::migrations::migrations()',
  'for migration_step in IndexModule.migrations()',
  'CREATE TABLE oauth_apps',
  'CREATE SCHEMA',
  'DROP SCHEMA IF EXISTS',
  '.register(IndexModule)',
  '.register(rustok_channel::ChannelModule)',
  '.register(rustok_product::ProductModule)',
  'rustok_distribution::build_runtime_extensions(&registry)',
  'PostgresSchemaRegistrationStore::new',
  'materialize_postgres_index_sources',
  'materialize_index_source_registry',
  'materialize_postgres_index_query_runtime',
  '.get::<ModuleWorkRegistrations>()',
  'let scheduler_a = ModuleWorkScheduler::new()',
  'let scheduler_b = ModuleWorkScheduler::new()',
  '.register_all(',
  '&HostRuntimeContext::new(database.host_a.clone())',
  '&scheduler_a',
  '&HostRuntimeContext::new(database.host_b.clone())',
  '&scheduler_b',
  'ProductSalesChannelIndexRelationConvergenceStore::new(database.host_a.clone())',
  '.claim(TENANT_ID, initial_generation, Duration::from_secs(1))',
  'ProductSalesChannelIndexRelationConvergenceWork::VisibilityRequest',
  'assert_eq!(runtime.scheduler_b.run_once().await?, 0)',
  'assert_active_lease',
  'tokio::time::sleep(Duration::from_millis(1_100)).await',
  'assert_eq!(runtime.scheduler_b.run_once().await?, 1)',
  'assert_reclaimed_progress',
  'run_scheduler_until_idle(&runtime.scheduler_b, 32)',
  'for _ in 0..maximum_iterations',
  'assert_no_relation_or_freshness(&database.writer, MALFORMED_PRODUCT_ID)',
  'VALID_AFTER_MALFORMED_PRODUCT_ID',
  'let delayed_visibility = load_product_mutation',
  'update_restricted_visibility(&database.writer, "beta")',
  'apply_product_mutation(&runtime, delayed_visibility)',
  'assert_materialized_source_version',
  'assert_product_visible(&runtime.query, RESTRICTED_PRODUCT_ID, false)',
  'run_scheduler_until_idle(&runtime.scheduler_a, 16)',
  'assert_eq!(visibility_membership, vec![BETA_CHANNEL_ID])',
  'rename_channel(&database.writer, ALPHA_CHANNEL_ID, "alpha-renamed")',
  'generation_after_alpha_rename',
  'unrestricted_relation_before',
  'unrestricted_projection_before',
  'assert_freshness_generation(',
  'rename_channel(&database.writer, BETA_CHANNEL_ID, "beta-renamed")',
  'generation_after_beta_rename',
  'restricted_relation_after_beta > restricted_relation_before_beta',
  'restricted_projection_after_beta > restricted_projection_before_beta',
  'assert!(restricted_membership_after_beta.is_empty())',
  'current_after_beta.source_version()',
  'assert_state_checkpoint(&database.writer, generation_after_beta_rename)',
  'channel_index_identity_generations',
  'product_sales_channel_index_relation_convergence_state',
  'product_sales_channel_index_relation_snapshots',
  'product_sales_channel_index_relation_freshness_snapshots',
  'product_index_graph_projection_snapshots',
  'index_entities',
]);
forbidMarkers(harnessPath, harness, [
  'FakeIndex',
  'FakeSource',
  'MockIndex',
  'MockSource',
  'INSERT INTO index_entities',
  'UPDATE index_entities',
  'DELETE FROM index_entities',
  'PostgresIndexQueryPort::new',
  'PostgresIndexQueryPort::with_admissions',
  'ProductSalesChannelRelationResolver::new',
  'ProductSalesChannelRelationConvergenceAdapter',
  'loop {',
]);

requireMarkers('crates/rustok-index/docs/m7-product-channel-convergence-postgres-harness.md', [
  'Status: `source_ready_execution_pending`',
  'two independent `ModuleWorkScheduler` hosts',
  'lease expiry',
  'rejected Product',
  'visibility alpha -> beta',
  'unchanged UUID membership',
  'changed membership',
  'query-inadmissible',
  'not been executed',
]);
requireMarkers('scripts/verify/verify-index-query-contract.mjs', [
  "'verify-index-product-channel-convergence-postgres-harness.mjs'",
]);

console.log('[verify-index-product-channel-convergence-postgres-harness] source packet verified');

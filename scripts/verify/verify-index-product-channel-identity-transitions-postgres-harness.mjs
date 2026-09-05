#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const read = (relative) => fs.readFileSync(path.join(root, relative), 'utf8');
const fail = (message) => {
  console.error(`[verify-index-product-channel-identity-transitions-postgres-harness] ${message}`);
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

const harnessPath = 'crates/rustok-distribution/tests/product_channel_identity_transitions_postgres.rs';
const harness = requireMarkers(harnessPath, [
  '#![cfg(feature = "mod-product")]',
  'RUSTOK_INDEX_TEST_DATABASE_URL',
  'rustok_channel::migrations::migrations()',
  'rustok_product::migrations::migrations()',
  'for migration_step in IndexModule.migrations()',
  "INSERT INTO tenants (id) VALUES ('{TENANT_A}'), ('{TENANT_B}')",
  '.register(IndexModule)',
  '.register(rustok_channel::ChannelModule)',
  '.register(rustok_product::ProductModule)',
  'rustok_distribution::build_runtime_extensions(&registry)',
  '.get::<ModuleWorkRegistrations>()',
  '.register_all(&HostRuntimeContext::new(database.work.clone()), &scheduler)',
  'run_scheduler_until_idle(&runtime.scheduler, 24)',
  'baseline_b_generation, 0',
  'materialize_current(&runtime, TENANT_A, PRODUCT_A)',
  'materialize_current(&runtime, TENANT_B, PRODUCT_B)',
  'insert_channel(&database.writer, TENANT_A, BETA_CHANNEL, "beta", "Beta")',
  'generation_after_create > baseline_a_generation',
  'a_relation_after_create > a_relation_before_create',
  'a_projection_after_create > a_projection_before_create',
  'vec![ALPHA_CHANNEL, BETA_CHANNEL]',
  'delete_channel(&database.writer, TENANT_A, BETA_CHANNEL)',
  'generation_after_delete > generation_after_create',
  'a_relation_after_delete > a_relation_before_delete',
  'a_projection_after_delete > a_projection_before_delete',
  'vec![ALPHA_CHANNEL]',
  'move_channel(&database.writer, ALPHA_CHANNEL, TENANT_A, TENANT_B)',
  'generation_a_after_move > generation_after_delete',
  'generation_b_after_move > baseline_b_generation',
  'a_membership_after_move.is_empty()',
  'b_relation_after_move > b_relation_before_move',
  'b_projection_after_move > b_projection_before_move',
  'vec![ALPHA_CHANNEL]',
  'delete_channel(&database.writer, TENANT_B, ALPHA_CHANNEL)',
  'insert_channel(',
  '&database.writer',
  'TENANT_B',
  'ALPHA_CHANNEL',
  '"alpha"',
  '"Alpha recreated"',
  'generation_after_identity_delete > generation_b_after_move',
  'generation_after_recreate > generation_after_identity_delete',
  'b_relation_after_recreate, b_relation_before_recreate',
  'b_projection_after_recreate, b_projection_before_recreate',
  'assert_freshness_generation(',
  'b_materialized_before_recreate',
  'materialized_source_version(&database.mutation, TENANT_B, PRODUCT_B)',
  'assert_product_visible(&runtime.query, TENANT_B, PRODUCT_B, true)',
  'assert_state_checkpoint(&database.writer, TENANT_A, generation_a_after_move)',
  'assert_state_checkpoint(&database.writer, TENANT_B, generation_after_recreate)',
  'channel_index_identity_generations',
  'product_sales_channel_index_relation_snapshots',
  'product_sales_channel_index_relation_freshness_snapshots',
  'product_index_graph_projection_snapshots',
  'product_sales_channel_index_relation_convergence_state',
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
  'tokio::spawn',
]);

requireMarkers('crates/rustok-index/docs/m7-product-channel-identity-transitions-postgres-harness.md', [
  'Status: `source_ready_execution_pending`',
  'Channel create',
  'Channel delete',
  'tenant move',
  'delete + recreate',
  'both tenant generations',
  'same materialized Product row',
  '`relation_epoch` does not advance',
  'not been executed',
]);
requireMarkers('scripts/verify/verify-index-query-contract.mjs', [
  "'verify-index-product-channel-identity-transitions-postgres-harness.mjs'",
]);

console.log('[verify-index-product-channel-identity-transitions-postgres-harness] source packet verified');

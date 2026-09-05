#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const read = (relative) => fs.readFileSync(path.join(root, relative), 'utf8');
const fail = (message) => {
  console.error(`[verify-index-product-channel-relation-convergence] ${message}`);
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

const migrationPath =
  'crates/rustok-product/src/migrations/m20260807_000012_add_product_sales_channel_relation_convergence.rs';
const migration = requireMarkers(migrationPath, [
  'CREATE TABLE product_sales_channel_index_relation_convergence_requests',
  'PRIMARY KEY (tenant_id, sequence_no)',
  'UNIQUE (tenant_id, product_id, product_source_version)',
  'CREATE TABLE product_sales_channel_index_relation_convergence_state',
  'visibility_cursor BIGINT NOT NULL DEFAULT 0',
  'channel_identity_generation BIGINT NULL',
  'sweep_generation BIGINT NULL',
  'sweep_after_product_id UUID NULL',
  'lease_token UUID NULL',
  'lease_expires_at TIMESTAMPTZ NULL',
  'available_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP',
  'attempt_count BIGINT NOT NULL DEFAULT 0',
  'last_error TEXT NULL',
  'Product-SalesChannel convergence requests are append-only',
  'state must start from the canonical empty checkpoint',
  'visibility cursor must advance exactly one leased request',
  'Channel generation may advance only by completing the leased sweep',
  'in-progress sweep generation is immutable',
  'completed sweep cursor must clear',
  'sweep cursor must advance strictly while completing a leased page',
  'sweep may clear only after checkpointing its generation',
  'availability may change only while releasing a lease',
  'lease cannot be acquired before availability',
  'lease acquisition must advance attempt count exactly once',
  'lease expiry may change only with lease ownership',
  'Product-SalesChannel convergence state cannot be deleted',
  'CREATE TRIGGER trg_product_channel_relation_convergence_state_insert_update',
  'BEFORE INSERT OR UPDATE ON product_sales_channel_index_relation_convergence_state',
  'CREATE OR REPLACE FUNCTION rustok_product_enqueue_channel_relation_convergence()',
  "IF TG_OP = 'INSERT' THEN",
  "OLD.metadata #> '{channel_visibility}'",
  "NEW.metadata #> '{channel_visibility}'",
  'NEW.index_revision',
  'AFTER INSERT ON products',
  'AFTER UPDATE OF metadata, tenant_id, id ON products',
  'SELECT DISTINCT tenant_id',
  'FROM products',
  'ON CONFLICT (tenant_id) DO NOTHING',
]);
forbidMarkers(migrationPath, migration, [
  'FROM channels',
  'JOIN channels',
  'channel_index_identity_generations',
  'index_entities',
  'index_links',
  'sys_events',
]);
requireMarkers('crates/rustok-product/src/migrations/mod.rs', [
  'mod m20260807_000012_add_product_sales_channel_relation_convergence;',
  'Box::new(m20260807_000012_add_product_sales_channel_relation_convergence::Migration)',
]);

const storePath = 'crates/rustok-product/src/services/index_channel_relation_convergence.rs';
const store = requireMarkers(storePath, [
  'pub enum ProductSalesChannelIndexRelationConvergenceWork',
  'VisibilityRequest {',
  'ChannelSweep {',
  'pub struct ProductSalesChannelIndexRelationConvergenceClaim',
  'pub fn restore(',
  'pub enum ProductSalesChannelIndexRelationConvergenceClaimOutcome',
  'pub struct ProductSalesChannelIndexRelationConvergenceStore',
  'pub async fn claim(',
  'pub async fn complete_visibility(',
  'pub async fn complete_sweep_page(',
  'pub async fn retry(',
  'FOR UPDATE',
  'load_next_visibility_request(transaction, tenant_id, state.visibility_cursor).await?',
  'state.channel_identity_generation < Some(observed_channel_identity_generation)',
  'sweep_generation = COALESCE(sweep_generation, $2)',
  'lease_expires_at = CURRENT_TIMESTAMP + ($4 * INTERVAL \'1 second\')',
  'attempt_count = attempt_count + 1',
  'visibility_cursor = $3',
  'channel_identity_generation = COALESCE($4, channel_identity_generation)',
  'available_at = CURRENT_TIMESTAMP + ($3 * INTERVAL \'1 second\')',
  'ProductSalesChannelIndexRelationConvergenceError::WatermarkRegressed',
  'ProductSalesChannelIndexRelationConvergenceError::LeaseLost',
]);
forbidMarkers(storePath, store, [
  'rustok_channel',
  'rustok_index',
  'FROM channels',
  'JOIN channels',
  'channel_index_identity_generations',
  'IndexMutation',
  'tokio::spawn',
  'loop {',
  'sys_events',
  'OutboxRelay',
]);
const productCargo = read('crates/rustok-product/Cargo.toml');
forbidMarkers('crates/rustok-product/Cargo.toml', productCargo, ['rustok-channel', 'rustok-index']);

requireMarkers('crates/rustok-product/src/services/mod.rs', [
  'mod index_channel_relation_convergence;',
  'ProductSalesChannelIndexRelationConvergenceStore',
  'ProductSalesChannelIndexRelationConvergenceWork',
]);
requireMarkers('crates/rustok-product/src/lib.rs', [
  'ProductSalesChannelIndexRelationConvergenceClaim',
  'ProductSalesChannelIndexRelationConvergenceStore',
  'ProductSalesChannelIndexRelationConvergenceWork',
]);

const workerPath =
  'crates/rustok-distribution/src/product_index/channel_relation_convergence.rs';
const worker = requireMarkers(workerPath, [
  'PRODUCT_SALES_CHANNEL_RELATION_CONVERGENCE_WORKER',
  'product_sales_channel_relation_convergence',
  'ModuleWorkRegistration',
  'ModuleWorkRegistrations',
  'ModuleWorkScheduler',
  'ModuleWorkSource',
  'ModuleWorkHandler',
  'rustok_product::ProductRuntimeSelected',
  'rustok_channel::ChannelRuntimeSelected',
  'product_sales_channel_index_relation_convergence_state',
  'channel_index_identity_generations',
  'product_sales_channel_index_relation_convergence_requests',
  'state.available_at <= CURRENT_TIMESTAMP',
  'state.sweep_generation IS NOT NULL',
  'state.channel_identity_generation IS NULL',
  'request.sequence_no > state.visibility_cursor',
  'LIMIT 1',
  '.claim(tenant_id, current_generation, LEASE_DURATION)',
  'ProductSalesChannelIndexRelationConvergenceClaim::restore',
  'async fn reconcile_sweep_page(',
  'reconcile_product(',
  'owner_rejected(&error)',
  'head-of-line block valid Products later in the same tenant',
  'ProductSalesChannelRelationResolverError::ProductNotFound',
  '.complete_visibility(&claim)',
  '.complete_sweep_page(&claim, next_product_id)',
  '.retry(&claim, delay, marker)',
  'RETRY_DELAY: Duration = Duration::from_secs(5)',
  'REJECTED_RETRY_DELAY: Duration = Duration::from_secs(60)',
  'LEASE_DURATION: Duration = Duration::from_secs(300)',
  'owner_rejection_isolated_from_retryable_storage_failures',
]);

requireMarkers('crates/rustok-distribution/src/product_index/channel_relation_resolver.rs', [
  'SELECT id FROM products WHERE tenant_id = $1',
  'reconcile_product(tenant_id, product_id)',
  'MAX_PRODUCT_SALES_CHANNEL_RELATION_RESOLVE_PAGE',
]);
forbidMarkers(workerPath, worker, [
  'tokio::spawn',
  'tokio::time::sleep',
  'loop {',
  'IndexMutation',
  'index_entities',
  'index_links',
  'sys_events',
  'OutboxRelay',
  'TransactionalEventBus',
]);

requireMarkers('crates/rustok-distribution/src/product_index/mod.rs', [
  'mod channel_relation_convergence;',
  'channel_relation_convergence::register(extensions)',
  'selected_product_and_channel_bridge_registers_channel_admission_and_convergence_work',
]);
requireMarkers('crates/rustok-distribution/Cargo.toml', ['rustok-runtime.workspace = true']);

requireMarkers('crates/rustok-product/docs/index-sales-channel-relation-convergence.md', [
  'Status: `source_complete_runtime_evidence_pending`',
  'is append-only and tenant ordered',
  'Channel generation sweep',
  'Lease and retry contract',
  'generic `ModuleWorkRegistration`',
  'rejected Product',
  'source-read -> mutation-apply',
]);
requireMarkers('crates/rustok-index/docs/m7-product-sales-channel-convergence.md', [
  'Status: `source_and_query_fence_complete_runtime_evidence_pending`',
  'Generic ModuleWork composition',
  'Multi-host and restart behavior',
  'Automatic relation convergence is now source complete',
  'materialized/query freshness fence is also source complete',
  'PostgreSQL execution evidence',
]);
requireMarkers('crates/rustok-index/docs/m7-product-sales-channel-resolver.md', [
  'Status: `automatic_convergence_and_query_fence_source_complete_runtime_evidence_pending`',
  'ProductSalesChannelIndexRelationFreshnessStore::record',
  'Automatic convergence composition',
  'Automatic convergence now re-establishes stale/missing relation freshness',
  'materialized/query freshness fence separately closes',
]);
requireMarkers('crates/rustok-index/docs/m7-product-sales-channel-relation-admission.md', [
  'Automatic Product visibility / Channel identity relation convergence through generic ModuleWork',
  'Materialized/query freshness admission for the source-read -> mutation-apply window: source complete',
  'PostgreSQL execution/admission evidence pending',
]);
requireMarkers('crates/rustok-index/docs/implementation-plan-current-2026-08-07.md', [
  'Product-owned visibility convergence requests and tenant lease/checkpoint state',
  'bounded generic ModuleWork Product-SalesChannel automatic convergence',
  'Automatic owner-change relation convergence source complete',
  'Materialized/query freshness fence source complete',
]);

const aggregate = read('scripts/verify/verify-index-query-contract.mjs');
if (!aggregate.includes("'verify-index-product-channel-relation-convergence.mjs'")) {
  fail('Index aggregate verifier does not include Product-SalesChannel convergence guard');
}

console.log('[verify-index-product-channel-relation-convergence] durable convergence and query-fence contract verified');

#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const read = (relative) => fs.readFileSync(path.join(root, relative), 'utf8');
const fail = (message) => {
  console.error(`[verify-index-linked-target-availability-equivalence-postgres-harness] ${message}`);
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

const harnessPath = 'crates/rustok-distribution/tests/product_linked_target_availability_equivalence_postgres.rs';
const harness = requireMarkers(harnessPath, [
  '#![cfg(feature = "mod-product")]',
  'linked_target_availability_preserves_filter_order_count_and_runtime_restart_parity',
  'rustok_channel::migrations::migrations()',
  'rustok_product::migrations::migrations()',
  'IndexModule.migrations()',
  '.register(IndexModule)',
  '.register(rustok_channel::ChannelModule)',
  '.register(rustok_product::ProductModule)',
  'PostgresSchemaRegistrationStore::new',
  'materialize_postgres_index_sources',
  'materialize_index_source_registry',
  'materialize_postgres_index_query_runtime',
  'PostgresMutationStore::new',
  'ModuleWorkRegistrations',
  'scheduler.run_once().await?',
  'PRODUCT_VARIANT_SOURCE: &str = "product-variant-postgres-primary"',
  'SALES_CHANNEL_SOURCE: &str = "sales-channel-postgres-primary"',
  'FilterExpr::In(',
  'direction: OrderDirection::MinAsc',
  'LinkName::new("variants")',
  'LinkName::new("sales_channels")',
  'update_variant_a_sku(&database.writer).await?',
  'variant_a_current > variant_a_materialized',
  'variant_stale_match_query()',
  'variant_current_match_query()',
  'let restarted_query = database.fresh_query_runtime().await?',
  'update_channel_a_name(&database.writer).await?',
  'generation_after_name_update, generation_before_name_update',
  'channel_a_current > channel_a_materialized',
  'channel_stale_match_query()',
  'channel_current_match_query()',
  'assert_ids_unordered(',
  'assert_ids_ordered(',
  '&[PRODUCT_B_ID]',
  '&[PRODUCT_A_ID, PRODUCT_B_ID]',
]);
forbidMarkers(harnessPath, harness, [
  'tokio::spawn',
  'loop {',
  'CREATE TABLE index_entities',
  'CREATE TABLE index_links',
  'PostgresQueryEntityAdmission::new',
  'register_postgres_index_query_link_target_availability',
]);

requireMarkers('crates/rustok-product/src/migrations/m20260730_000002_add_product_variant_index_revision.rs', [
  'BEFORE UPDATE ON product_variants',
  'NEW.index_revision := OLD.index_revision + 1',
]);
requireMarkers('crates/rustok-product/src/migrations/m20260731_000003_bump_product_index_revision_for_variant_membership.rs', [
  'AFTER UPDATE OF id, tenant_id, product_id ON product_variants',
  'OLD.id IS NOT DISTINCT FROM NEW.id',
]);
requireMarkers('crates/rustok-channel/src/migrations/m20260730_000010_add_channel_index_revision.rs', [
  'BEFORE UPDATE ON channels',
  'NEW.index_revision := OLD.index_revision + 1',
]);
requireMarkers('crates/rustok-channel/src/migrations/m20260807_000012_add_channel_index_identity_generation.rs', [
  'AFTER INSERT OR DELETE OR UPDATE OF id, tenant_id, slug ON channels',
]);
requireMarkers('crates/rustok-index/src/infrastructure/postgres/query_admission.rs', [
  'query.referenced_paths()',
  'path.links().first()',
  'apply_root_predicate(&mut compiled.sql, &predicate)?',
  'compiled.exact_count.as_mut()',
  '{link}.source_version = {root}.source_version',
  'owner_dispatch_for_alias(&owner_rules, AVAILABILITY_TARGET_ALIAS)',
]);

console.log('[verify-index-linked-target-availability-equivalence-postgres-harness] source-ready equivalence packet verified');

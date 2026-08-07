#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const read = (relative) => fs.readFileSync(path.join(root, relative), 'utf8');
const fail = (message) => {
  console.error(`[verify-index-linked-target-recreate-postgres-harness] ${message}`);
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

const harnessPath = 'crates/rustok-distribution/tests/product_linked_target_recreate_postgres.rs';
const harness = requireMarkers(harnessPath, [
  '#![cfg(feature = "mod-product")]',
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
  'ModuleWorkScheduler::new()',
  'scheduler.run_once().await?',
  'PRODUCT_SOURCE: &str = "product-postgres-primary"',
  'PRODUCT_VARIANT_SOURCE: &str = "product-variant-postgres-primary"',
  'SALES_CHANNEL_SOURCE: &str = "sales-channel-postgres-primary"',
  'FieldPath::linked(',
  'LinkName::new("variants")',
  'LinkName::new("sales_channels")',
  'delete_variant(&database.writer).await?',
  'variant_tombstone_version(&database.writer)',
  'recreate_variant(&database.writer).await?',
  'recreated_variant_version > variant_tombstone',
  'variant_tombstone_version(&database.writer).await?.is_none()',
  'assert_materialized_target_version(',
  '"product_variant"',
  'assert_graph_payloads(&runtime.query, &[], &[OLD_CHANNEL_NAME])',
  'delete_channel(&database.writer).await?',
  'channel_tombstone_version(&database.writer)',
  'recreate_channel(&database.writer).await?',
  'recreated_channel_version > channel_tombstone',
  'channel_tombstone_version(&database.writer).await?.is_none()',
  'assert_product_root_visible(&runtime.query, false)',
  'run_scheduler_until_idle(&runtime.scheduler, 20)',
  'latest_relation_epoch(&database.writer)',
  'latest_projection_epoch(&database.writer)',
  'latest_freshness_generation(&database.writer)',
  '"sales_channel"',
  'assert_graph_payloads(&runtime.query, &[NEW_VARIANT_SKU], &[])',
  'assert_graph_payloads(',
  '&[NEW_VARIANT_SKU]',
  '&[NEW_CHANNEL_NAME]',
]);
forbidMarkers(harnessPath, harness, [
  'tokio::spawn',
  'loop {',
  'CREATE TABLE product_variant_index_tombstones',
  'CREATE TABLE channel_index_tombstones',
  'ALTER TABLE product_variants',
  'ALTER TABLE channels',
  'PostgresQueryEntityAdmission::new',
]);

requireMarkers(
  'crates/rustok-product/src/migrations/m20260731_000004_add_product_index_tombstones.rs',
  [
    'OLD.index_revision + 1',
    'rustok_product_variant_seed_index_revision_from_tombstone',
    'retained_source_version + 1',
    'rustok_product_variant_clear_inserted_index_tombstone',
  ],
);
requireMarkers(
  'crates/rustok-channel/src/migrations/m20260731_000011_add_channel_index_tombstones.rs',
  [
    'OLD.index_revision + 1',
    'rustok_channel_seed_index_revision_from_tombstone',
    'retained_source_version + 1',
    'rustok_channel_clear_inserted_index_tombstone',
  ],
);
requireMarkers('crates/rustok-distribution/src/product_index/query_admission.rs', [
  'owner_variant.index_revision = {{entity}}.source_version',
  'owner_channel.index_revision = {{entity}}.source_version',
]);
requireMarkers('crates/rustok-index/docs/m7-linked-target-recreate-postgres-harness.md', [
  'Status: `source_ready_execution_pending`',
  'It adds no owner clock, no Index schema, and no compatibility version.',
  'ProductVariant scenario',
  'SalesChannel scenario',
  'link that exists while its target',
  'does not claim execution success',
]);

console.log('[verify-index-linked-target-recreate-postgres-harness] source-ready packet verified');

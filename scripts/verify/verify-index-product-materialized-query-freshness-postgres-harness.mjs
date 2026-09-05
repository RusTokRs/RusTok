#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const read = (relative) => fs.readFileSync(path.join(root, relative), 'utf8');
const fail = (message) => {
  console.error(`[verify-index-product-materialized-query-freshness-postgres-harness] ${message}`);
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

const harnessPath = 'crates/rustok-distribution/tests/product_materialized_query_freshness_postgres.rs';
const harness = requireMarkers(harnessPath, [
  '#![cfg(feature = "mod-product")]',
  'RUSTOK_INDEX_TEST_DATABASE_URL',
  'struct ProductMigrator;',
  'rustok_product::migrations::migrations()',
  'for migration_step in IndexModule.migrations()',
  'CREATE SCHEMA',
  'DROP SCHEMA IF EXISTS',
  '.register(IndexModule)',
  '.register(rustok_channel::ChannelModule)',
  '.register(rustok_product::ProductModule)',
  'rustok_distribution::build_runtime_extensions(&registry)',
  'PostgresSchemaRegistrationStore::new',
  '.register(TENANT_ID, &registered.schema)',
  'materialize_postgres_index_sources',
  'materialize_index_source_registry',
  'materialize_postgres_index_query_runtime',
  'SharedIndexSourceRegistry',
  'IndexSourceLoadRequest::new',
  'sources.load(request).await?',
  'PostgresMutationStore::new',
  'MutationDelivery::from_event(PRODUCT_SOURCE, mutation)',
  '.apply(runtime.schemas.registry(), &delivery)',
  'let delayed = load_product_mutation',
  'let delayed_source_version = delayed.source_version()',
  'bump_stale_product_owner_revision(&database.writer).await?',
  'assert_owner_projection_advanced(',
  'FROM product_index_graph_projection_snapshots',
  'projection_epoch > delayed_source_version',
  'apply_product_mutation(&runtime, delayed).await?',
  'assert_materialized_source_version(',
  'FROM index_entities',
  'FilterExpr::In(',
  'OrderDirection::Asc',
  'Pagination::Cursor { first: 1, after }',
  'include_exact_count: true',
  'assert_eq!(fenced.items[0].entity_id, CONTROL_PRODUCT_ID)',
  'assert_eq!(fenced.exact_count, Some(1))',
  'assert!(fenced.next_cursor.is_none())',
  'assert!(current.source_version() > delayed_source_version)',
  'assert_eq!(first.items[0].entity_id, STALE_PRODUCT_ID)',
  'assert_eq!(first.exact_count, Some(2))',
  'let cursor = first',
  '.next_cursor',
  'product_title_page(Some(cursor))',
  'assert_eq!(second.items[0].entity_id, CONTROL_PRODUCT_ID)',
  'let locale_delayed =',
  'delete_product_locale(&database.writer).await?',
  'apply_product_mutation(&runtime, locale_delayed).await?',
  'product_identity_query(LOCALE_DELETE_PRODUCT_ID)',
  'assert!(deleted_locale.items.is_empty())',
  'assert_eq!(deleted_locale.exact_count, Some(0))',
  'UPDATE products SET vendor =',
  'DELETE FROM product_translations',
  'product_sales_channel_index_relation_freshness_snapshots',
  'channel_index_identity_generations',
]);
forbidMarkers(harnessPath, harness, [
  'tokio::spawn',
  'tokio::time::sleep',
  'FakeIndex',
  'FakeSource',
  'MockIndex',
  'MockSource',
  'INSERT INTO index_entities',
  'UPDATE index_entities',
  'DELETE FROM index_entities',
  'PostgresIndexQueryPort::new',
  'PostgresIndexQueryPort::with_admissions',
]);

requireMarkers('crates/rustok-index/docs/m7-product-materialized-query-freshness-postgres-harness.md', [
  'Status: `source_ready_execution_pending`',
  'delayed scalar mutation',
  'physically present in `index_entities`',
  'filter/order/cursor/limit/exact-count',
  'corrective current mutation',
  'locale deletion',
  'not been executed',
  'Channel-generation and visibility convergence races',
]);

requireMarkers('scripts/verify/verify-index-query-contract.mjs', [
  "'verify-index-product-materialized-query-freshness-postgres-harness.mjs'",
]);

console.log('[verify-index-product-materialized-query-freshness-postgres-harness] source packet verified');

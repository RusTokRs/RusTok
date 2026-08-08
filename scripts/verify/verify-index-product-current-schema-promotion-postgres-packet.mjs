#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const read = (relative) => fs.readFileSync(path.join(root, relative), 'utf8');
const fail = (message) => {
  console.error(`[verify-index-product-current-schema-promotion-postgres-packet] ${message}`);
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

const packetPath = 'crates/rustok-distribution/tests/product_current_schema_promotion_postgres.rs';
const packet = requireMarkers(packetPath, [
  '#![cfg(feature = "mod-product")]',
  'RUSTOK_PRODUCT_KEY4_PROMOTION_DATABASE_URL',
  'PRODUCT_SOURCE: &str = "product-postgres-primary"',
  'PRODUCT_EVENT_DOMAIN: &str = "rustok-product.product-replay"',
  'CURRENT_PRODUCT_SCHEMA_VERSION: u32 = 4',
  'HISTORICAL_PRODUCT_SCHEMA_VERSION: u32 = 3',
  'rustok_product::migrations::migrations()',
  'for migration_step in IndexModule.migrations()',
  'rustok_distribution::build_runtime_extensions(&registry)?',
  '.register(rustok_channel::ChannelModule)',
  '.register(rustok_product::ProductModule)',
  'current_product_schema(&schemas)?',
  'let mut historical_product_schema = current_product_schema.clone();',
  'historical_product_schema.reference.version =',
  'schema_store.register(TENANT_ID, &historical_product_schema)',
  'for registered in schemas.registry().iter()',
  'materialize_postgres_index_sources(&mut extensions, database.source.clone())?',
  'materialize_index_source_registry(&extensions)?',
  'materialize_postgres_index_query_runtime(&mut extensions, database.query.clone())?',
  'load_product_mutation(&runtime.sources, &current_ref)',
  'derive_index_schema_source_event_id(',
  'assert_ne!(current_event_id, historical_event_id);',
  'MutationDelivery::from_event(PRODUCT_SOURCE, mutation)?',
  '.apply(runtime.schemas.registry(), &delivery)',
  '.register_current(TENANT_ID, &runtime.current_product_schema)',
  'assert_eq!(promoted.retired_schema_count(), 1);',
  'assert_eq!(repeated.retired_schema_count(), 0);',
  'PostgresIndexSchemaReadinessStore::new(database.query.clone())',
  'PersistedSchemaReadinessFailure::Inactive',
  'PostgresIndexQueryPort::new(database.query.clone(), probe_registry)',
  'IndexQueryExecutionError::SchemaNotReady { reference, reason }',
  'restart_current_product_query_runtime(database).await?',
  'assert_eq!(product_schemas.len(), 1);',
  'SchemaVersion::new(CURRENT_PRODUCT_SCHEMA_VERSION)',
  'assert_current_product_query(&restart_query, &current_ref).await?;',
  'CREATE SCHEMA',
  'DROP SCHEMA IF EXISTS',
]);

forbidMarkers(packetPath, packet, [
  'SchemaVersion::new(5)',
  'product_v3_schema',
  'product_v4_schema',
  'PRODUCT_EVENT_DOMAIN_V3',
  'PRODUCT_EVENT_DOMAIN_V4',
  'derive_index_source_event_id(',
  'mod product_v3',
  'SharedIndexSourceRegistry::new',
  'tokio::spawn',
]);

// The lower-key contract is deliberately a storage/probe fixture. The packet must never register a key3
// source factory or add key3 to the selected distribution runtime.
const distributionProductPath = 'crates/rustok-distribution/src/product_index/product.rs';
const distributionProduct = requireMarkers(distributionProductPath, [
  'derive_index_schema_source_event_id',
  'SchemaVersion::new(PRODUCT_SCHEMA_ROUTING_KEY)',
]);
forbidMarkers(distributionProductPath, distributionProduct, [
  'SchemaVersion::new(3)',
  'product_v3_schema',
  'derive_index_source_event_id(',
]);

requireMarkers('crates/rustok-distribution/src/product_index/mod.rs', [
  'PRODUCT_SCHEMA_ROUTING_KEY: u32 = 4',
  'Lower keys are historical storage identities only.',
]);
requireMarkers('crates/rustok-index/docs/m7-product-current-schema-promotion.md', [
  'Status: `postgres_packet_source_complete_execution_pending`',
  'Retained PostgreSQL promotion packet — source complete',
  'storage-only lower-key fixture',
  'does not reconstruct or select the historical key3 Product implementation',
  'Maintainer execution still required',
]);

console.log('[verify-index-product-current-schema-promotion-postgres-packet] retained Product key4 stage/replay/register_current/inactive-old-key/restart PostgreSQL packet source verified; execution remains maintainer-owned');

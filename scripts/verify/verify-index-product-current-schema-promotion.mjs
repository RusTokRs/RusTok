#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const read = (relative) => fs.readFileSync(path.join(root, relative), 'utf8');
const fail = (message) => {
  console.error(`[verify-index-product-current-schema-promotion] ${message}`);
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

const modulePath = 'crates/rustok-distribution/src/product_index/mod.rs';
const moduleSource = requireMarkers(modulePath, [
  'PRODUCT_SCHEMA_ROUTING_KEY: u32 = 4',
  'Lower keys are historical storage identities only.',
]);
forbidMarkers(modulePath, moduleSource, [
  'PRODUCT_SCHEMA_ROUTING_KEY: u32 = 3',
  'PRODUCT_SCHEMA_ROUTING_KEY: u32 = 5',
  'mod product_v3',
  'mod product_v4',
]);

const productPath = 'crates/rustok-distribution/src/product_index/product.rs';
const product = requireMarkers(productPath, [
  'PRODUCT_INDEX_SOURCE: &str = "product-postgres-primary"',
  'derive_index_schema_source_event_id',
  'SchemaVersion::new(PRODUCT_SCHEMA_ROUTING_KEY)',
  'assert_eq!(schema.fields.len(), 15);',
  'assert_eq!(schema.links.len(), 2);',
]);
forbidMarkers(productPath, product, [
  'derive_index_source_event_id(',
  'SchemaVersion::new(3)',
  'product_v3_schema',
  'product_v4_schema',
]);

for (const currentConsumer of [
  'crates/rustok-distribution/src/product_index/absence.rs',
  'crates/rustok-distribution/src/product_index/query_admission.rs',
]) {
  const consumer = requireMarkers(currentConsumer, [
    'PRODUCT_SCHEMA_ROUTING_KEY',
    'SchemaVersion::new(PRODUCT_SCHEMA_ROUTING_KEY)',
  ]);
  forbidMarkers(currentConsumer, consumer, ['SchemaVersion::new(3)']);
}

const registrationPath = 'crates/rustok-index/src/infrastructure/postgres/schema_registration.rs';
const registration = requireMarkers(registrationPath, [
  'pub async fn register_current(',
  'register_current_in_transaction(',
  'retire_lower_active_schemas(',
  'let version: i32 = row.try_get("", "schema_version").map_err(storage_error)?;',
  "status = 'retired'",
  'schema_version < $4 AND status = \'active\'',
  'Historical entity/link/inbox/replay rows are not deleted or rewritten',
]);
forbidMarkers(registrationPath, registration, [
  'DELETE FROM index_schemas',
  'DELETE FROM index_entities',
  'DELETE FROM index_links',
  'UPDATE index_entities SET schema_version',
]);

requireMarkers('crates/rustok-index/src/infrastructure/postgres/schema_registration_tests.rs', [
  'ordinary_registration_does_not_implicitly_retire_older_contracts',
  'staged_latest_contract_can_be_promoted_without_reinsertion',
  'explicit_current_supersession_retires_all_lower_active_contracts',
  'historical_exact_contract_cannot_be_declared_current_after_supersession',
  'supersession_is_tenant_scoped',
]);

requireMarkers('crates/rustok-index/src/infrastructure/postgres/schema_readiness.rs', [
  'let schema_version: i32 = row.try_get("", "schema_version").map_err(storage_error)?;',
  'let reason = if persisted.status != "active" {',
  'Some(PersistedSchemaReadinessFailure::Inactive)',
]);
requireMarkers('crates/rustok-index/src/infrastructure/postgres/query_port.rs', [
  'if status != "active"',
  'PersistedSchemaReadinessFailure::Inactive',
]);
requireMarkers('crates/rustok-index/src/application/source_event_id.rs', [
  'pub fn derive_index_schema_source_event_id(',
  'rustok-index-schema-source-event-id-v1',
]);

requireMarkers('crates/rustok-index/docs/m4-single-current-schema-supersession.md', [
  'Current Product key-4 application',
  'current Product key `4` contract',
  'The runtime must not stage or select a Product key `3` implementation',
]);
requireMarkers('crates/rustok-index/docs/m7-product-current-schema-promotion.md', [
  'Status: `postgres_packet_source_complete_execution_pending`',
  'Current source identity',
  'Tenant promotion sequence',
  'ordinary-register the exact Product key `4` immutable contract',
  '`PostgresSchemaRegistrationStore::register_current`',
  'Historical state',
  'Retained PostgreSQL promotion packet — source complete',
  'storage-only lower-key',
  'Mounted Storefront remains owner-native',
  'Maintainer execution still required',
]);
requireMarkers('scripts/verify/verify-index-product-current-schema-promotion-postgres-packet.mjs', [
  'retained Product key4 stage/replay/register_current/inactive-old-key/restart PostgreSQL packet source verified',
]);

console.log('[verify-index-product-current-schema-promotion] Product key4 is the only current runtime contract; retained tenant promotion PostgreSQL packet is source-complete and execution remains maintainer-owned');

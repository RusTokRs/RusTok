#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const read = (relative) => fs.readFileSync(path.join(root, relative), 'utf8');
const fail = (message) => {
  console.error(`[verify-index-schema-supersession] ${message}`);
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

const storePath = 'crates/rustok-index/src/infrastructure/postgres/schema_registration.rs';
const store = requireMarkers(storePath, [
  'pub struct PersistedSchemaSupersessionOutcome',
  'pub fn registration(&self) -> &PersistedSchemaRegistrationOutcome',
  'pub fn retired_schema_count(&self) -> u64',
  'pub async fn register_current(',
  'register_current_in_transaction(',
  'let latest = load_latest_version(',
  'schema.reference.version < latest',
  'SchemaRegistrationError::NonMonotonicVersion',
  'resolve_existing_schema(schema, fingerprint, schema_json, existing)?',
  'retire_lower_active_schemas(',
  'status = \'retired\'',
  'schema_version < $4 AND status = \'active\'',
  'schema_version < ?4 AND status = \'active\'',
  'updated_at = CURRENT_TIMESTAMP',
  'Historical entity/link/inbox/replay rows are not deleted or rewritten',
]);
forbidMarkers(storePath, store, [
  'DELETE FROM index_schemas',
  'DELETE FROM index_entities',
  'DELETE FROM index_links',
  'UPDATE index_entities SET schema_version',
  'UPDATE index_links SET source_schema_version',
  'rustok-product',
]);

const testsPath = 'crates/rustok-index/src/infrastructure/postgres/schema_registration_tests.rs';
requireMarkers(testsPath, [
  'ordinary_registration_does_not_implicitly_retire_older_contracts',
  'staged_latest_contract_can_be_promoted_without_reinsertion',
  'explicit_current_supersession_retires_all_lower_active_contracts',
  'explicit_current_supersession_is_idempotent_for_latest_current_contract',
  'historical_exact_contract_cannot_be_declared_current_after_supersession',
  'supersession_is_tenant_scoped',
  'promoted.registration()',
  'PersistedSchemaRegistrationOutcome::Unchanged { .. }',
  'promoted.retired_schema_count(), 1',
  'outcome.retired_schema_count(), 2',
  'repeated.retired_schema_count(), 0',
  'schema_status(&db, TENANT_A, 1).await, "retired"',
  'schema_status(&db, TENANT_A, 3).await, "active"',
]);

requireMarkers('crates/rustok-index/src/infrastructure/postgres/mod.rs', [
  'PersistedSchemaSupersessionOutcome',
]);
requireMarkers('crates/rustok-index/src/lib.rs', [
  'PersistedSchemaSupersessionOutcome',
]);

requireMarkers('crates/rustok-index/src/infrastructure/postgres/schema_readiness.rs', [
  'if status != "active"',
  'IndexSchemaReadinessFailure::Inactive',
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
  'Status: `source_complete_execution_pending`',
  '`PostgresSchemaRegistrationStore::register_current`',
  'Inbox delivery identity is a separate boundary',
  '`(tenant_id, source_name, delivery_id)`',
  '`derive_index_schema_source_event_id`',
  'Recommended staged rebuild sequence',
  'use ordinary `register` to stage',
  'call `register_current` with that already-staged exact contract',
  'Historical rows may be purged later',
  'Current Product key-4 application',
  'publishes exactly one 15-field Product contract on routing key `4`',
  'uses `derive_index_schema_source_event_id`',
  'must not stage or select a Product key `3` implementation',
  'm7-product-current-schema-promotion.md',
]);

requireMarkers('scripts/verify/verify-index-product-current-schema-promotion.mjs', [
  'Product key4 is the only current runtime contract',
]);

console.log('[verify-index-schema-supersession] explicit single-current staged schema supersession source contract verified with current Product key4 application');

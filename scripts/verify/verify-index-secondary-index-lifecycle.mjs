#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const read = (relative) => fs.readFileSync(path.join(root, relative), 'utf8');
const fail = (message) => {
  console.error(`[verify-index-secondary-index-lifecycle] ${message}`);
  process.exit(1);
};
const requireMarkers = (relative, markers) => {
  const source = read(relative);
  for (const marker of markers) {
    if (!source.includes(marker)) fail(`${relative} is missing ${marker}`);
  }
  return source;
};

const lifecyclePath = 'crates/rustok-index/src/infrastructure/postgres/secondary_index.rs';
const lifecycle = requireMarkers(lifecyclePath, [
  'pub struct SecondaryIndexSpec',
  'pub struct SecondaryIndexPlan',
  'pub enum SecondaryIndexOperation',
  'pub struct SecondaryIndexRequest',
  'pub struct SecondaryIndexLease',
  'pub struct PostgresSecondaryIndexManager',
  'tagged_index_value_v1',
  'FieldCardinality::One => SecondaryIndexKind::Scalar',
  'FieldCardinality::Many => SecondaryIndexKind::JsonContainment',
  'field.filterable || field.sortable',
  'CREATE INDEX CONCURRENTLY',
  "(payload -> {field}) ->> 'value'",
  "USING gin (((payload -> {field}) -> 'value') jsonb_path_ops)",
  'regexp_replace({value}',
  'COLLATE \\"C\\"',
  'REINDEX INDEX CONCURRENTLY',
  'DROP INDEX CONCURRENTLY IF EXISTS',
  'schema_fingerprint =',
  'is_deleted = FALSE',
  'kind = \'secondary_index\'',
  'scope_kind = \'schema\'',
  'pg_advisory_xact_lock(hashtextextended($1, 0))',
  'lease_expires_at <= CURRENT_TIMESTAMP',
  'lease_expires_at > CURRENT_TIMESTAMP',
  'attempt_count =',
  'obj_description(index_class.oid, \'pg_class\')',
  'indisready',
  'indisvalid',
  'rustok-index:',
  'IndexOwnershipConflict',
  'IndexNotReady',
  'LeaseLost',
  'operation != SecondaryIndexOperation::Retire',
  'DbBackend::Sqlite if cfg!(test)',
]);

const lockPosition = lifecycle.indexOf('self.lock_index(transaction, request.spec(), backend)');
const schemaPosition = lifecycle.indexOf('self.verify_schema_registration(');
const jobPosition = lifecycle.indexOf('select_active_jobs_sql(backend)');
if (
  lockPosition < 0 ||
  schemaPosition < 0 ||
  jobPosition < 0 ||
  !(lockPosition < schemaPosition && schemaPosition < jobPosition)
) {
  fail('index advisory lock, persisted schema verification, and active-job selection must remain ordered');
}

for (const forbidden of [
  'rustok_product',
  'rustok_content',
  'rustok_flex',
  'rustok_pricing',
  'rustok_inventory',
  'CREATE TABLE product',
  'VACUUM FULL',
  'DROP TABLE index_entities',
  "payload ->> {field}",
]) {
  if (lifecycle.includes(forbidden)) fail(`${lifecyclePath} contains forbidden marker ${forbidden}`);
}

requireMarkers('crates/rustok-index/src/infrastructure/postgres/secondary_index_tests.rs', [
  'plan_derives_stable_typed_and_containment_indexes',
  'ensure_reindex_and_retire_are_durable_and_idempotent',
  'expired_operation_is_reclaimed_with_attempt_fencing',
  'schema_and_request_validation_fail_closed',
  "(payload -> 'price_minor') ->> 'value'",
  "((payload -> 'tags') -> 'value') jsonb_path_ops",
  'retirement should remain available',
  'SecondaryIndexClaimOutcome::Busy',
  'SecondaryIndexError::LeaseLost',
]);
requireMarkers('crates/rustok-index/src/infrastructure/postgres/mod.rs', [
  'mod secondary_index;',
  'mod secondary_index_tests;',
  'PostgresSecondaryIndexManager',
  'SecondaryIndexPlan',
]);
requireMarkers('crates/rustok-index/src/lib.rs', [
  'PostgresSecondaryIndexManager',
  'SecondaryIndexExecutionOutcome',
  'SecondaryIndexSpec',
]);
requireMarkers('crates/rustok-index/docs/implementation-plan.md', [
  '- [x] Add secondary-index planning and lifecycle management.',
  'M3 secondary-index lifecycle: `complete`',
]);
requireMarkers('DECISIONS/2026-07-24-index-storage-layout.md', [
  'Typed filtering and ordering use deterministic schema-managed expression indexes',
  'built concurrently through an observable index-management job',
]);

console.log('[verify-index-secondary-index-lifecycle] OK');

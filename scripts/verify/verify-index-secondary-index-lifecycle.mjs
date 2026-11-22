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

const lifecyclePath = 'crates/modules/rustok-index/src/infrastructure/postgres/secondary_index.rs';
const lifecycle = requireMarkers(lifecyclePath, [
  'pub struct SecondaryIndexSpec',
  'pub struct SecondaryIndexPlan',
  'pub enum SecondaryIndexOperation',
  'pub struct SecondaryIndexRequest',
  'pub struct SecondaryIndexLease',
  'pub struct PostgresSecondaryIndexManager',
  'tagged_index_expression_v1',
  'FieldCardinality::One => SecondaryIndexKind::Scalar',
  'FieldCardinality::Many => SecondaryIndexKind::JsonContainment',
  'field.filterable || field.sortable',
  'CREATE INDEX CONCURRENTLY',
  'scalar_expression("payload", &self.field_name, self.value_type)',
  'list_expression("payload", &self.field_name)',
  'jsonb_path_ops',
  'IndexValueType::Decimal',
  'REINDEX INDEX CONCURRENTLY',
  'DROP INDEX CONCURRENTLY IF EXISTS',
  'kind = \'secondary_index\'',
  'scope_kind = \'schema\'',
  'pg_try_advisory_xact_lock(hashtextextended($1, 0))',
  'due_job_where(backend, "secondary_index")',
  'live_lease_where(backend, "secondary_index", 4, false)',
  'attempt_count =',
  "obj_description(c.oid, 'pg_class')",
  'connection.close_on_drop()',
  'pg_try_advisory_lock(hashtextextended($1, 0))',
  'creating_index',
  'table_oid',
  'indisready',
  'indisvalid',
  'rustok-index:',
  'IndexOwnershipConflict',
  'IndexNotReady',
  'LeaseLost',
  'operation != SecondaryIndexOperation::Retire',
  'DbBackend::Sqlite if cfg!(test)',
]);

requireMarkers('crates/modules/rustok-index/src/application/postgres_value_sql.rs', [
  'pub(crate) fn scalar_expression(',
  'left({text}, -1)',
  'pub(crate) fn schema_scope_sql(',
  'schema_fingerprint =',
  'is_deleted = FALSE',
]);
requireMarkers('apps/server/src/graphql/index_replay.rs', [
  'async fn maintain_index_secondary_index(',
  'SharedIndexSecondaryIndexRuntime',
  'runtime.maintain(authority, &schema, &field, operation)',
]);

const startSession = lifecycle.indexOf('async fn execute_postgres(');
const closeOnDrop = lifecycle.indexOf('connection.close_on_drop()', startSession);
const sessionLock = lifecycle.indexOf('pg_try_advisory_lock(hashtextextended($1, 0))', closeOnDrop);
const ddl = lifecycle.indexOf('execute_postgres_locked(&mut connection, lease)', sessionLock);
if (startSession < 0 || closeOnDrop <= startSession || sessionLock <= closeOnDrop || ddl <= sessionLock) {
  fail('PostgreSQL DDL needs one non-reusable session, even if the request is dropped');
}
requireMarkers(lifecyclePath, [
  'if stored.creation_reserved {',
  'Some(cursor) if cursor == creation_intent(lease, table_oid)',
  'index.definition != expected',
  'reserve_creation(session, lease, table_oid, &qualified_index)',
  'live_lease_where(DbBackend::Postgres, "secondary_index", 1, false)',
  'WHERE {fence} AND cursor IS NULL',
  'WHERE {due} AND cursor IS NULL',
]);
requireMarkers('crates/modules/rustok-index/src/infrastructure/postgres/postgres_reference_equivalence_tests/hardening.rs', [
  'postgres_ensure_only_recovers_unmarked_create_with_original_job_intent',
  'dropped_postgres_ensure_can_be_reclaimed_without_reusing_its_ddl_session',
  'SELECT j.job_id, l.pid FROM index_jobs AS j',
  'pg_catalog.pg_stat_activity WHERE pid = $1',
  '.bind(discarded_pid)',
  'postgres_compiled_queries_use_owned_btree_and_gin_in_custom_and_generic_plans',
]);

const lockPosition = lifecycle.indexOf('.lock_index(transaction, request.spec(), backend)');
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

requireMarkers('crates/modules/rustok-index/src/infrastructure/postgres/secondary_index_tests.rs', [
  'plan_derives_stable_typed_and_containment_indexes',
  'ensure_reindex_and_retire_are_durable_and_idempotent',
  'expired_operation_is_reclaimed_with_attempt_fencing',
  'schema_and_request_validation_fail_closed',
  "(payload -> 'price_minor') ->> 'value'",
  "USING gin ((((payload -> 'tags') -> 'value')) jsonb_path_ops)",
  'retirement should remain available',
  'SecondaryIndexClaimOutcome::Busy',
  'SecondaryIndexError::LeaseLost',
]);
requireMarkers('crates/modules/rustok-index/src/infrastructure/postgres/mod.rs', [
  'mod secondary_index;',
  'mod secondary_index_tests;',
  'PostgresSecondaryIndexManager',
  'SecondaryIndexPlan',
]);
requireMarkers('crates/modules/rustok-index/src/lib.rs', [
  'PostgresSecondaryIndexManager',
  'SecondaryIndexExecutionOutcome',
  'SecondaryIndexSpec',
]);
requireMarkers('crates/modules/rustok-index/docs/implementation-plan.md', [
  '- [x] Add secondary-index planning and lifecycle management.',
  'M3 secondary-index lifecycle: `complete`',
]);
requireMarkers('DECISIONS/2026-07-24-index-storage-layout.md', [
  'Typed filtering and ordering use deterministic schema-managed expression indexes',
  'built concurrently through an observable index-management job',
]);

console.log('[verify-index-secondary-index-lifecycle] OK');

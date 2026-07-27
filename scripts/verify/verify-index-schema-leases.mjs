#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const read = (relative) => fs.readFileSync(path.join(root, relative), 'utf8');
const fail = (message) => {
  console.error(`[verify-index-schema-leases] ${message}`);
  process.exit(1);
};
const requireMarkers = (relative, markers) => {
  const source = read(relative);
  for (const marker of markers) {
    if (!source.includes(marker)) fail(`${relative} is missing ${marker}`);
  }
  return source;
};

const leasePath = 'crates/rustok-index/src/infrastructure/postgres/schema_lease.rs';
const lease = requireMarkers(leasePath, [
  'pub struct SchemaApplicationLeaseRequest',
  'pub struct SchemaApplicationLease',
  'pub enum SchemaLeaseAcquireOutcome',
  'pub struct PostgresSchemaLeaseStore',
  'pub async fn acquire(',
  'pub async fn heartbeat(',
  'pub async fn succeed(',
  'pub async fn fail(',
  'pg_advisory_xact_lock(hashtextextended($1, 0))',
  "kind = 'schema_apply'",
  "scope_kind = 'schema'",
  "state IN ('pending', 'running', 'succeeded')",
  "lease_expires_at <= CURRENT_TIMESTAMP",
  "lease_expires_at > CURRENT_TIMESTAMP",
  'attempt_count =',
  'SchemaLeaseError::LeaseLost',
  'SchemaLeaseAcquireOutcome::AlreadyApplied',
  'SchemaLeaseAcquireOutcome::Busy',
  'schema_fingerprint',
  'DbBackend::Sqlite if cfg!(test) => Ok(())',
]);

const lockPosition = lease.indexOf('self.lock_schema(transaction, request, backend)');
const verifyPosition = lease.indexOf('self.verify_schema_registration(transaction, request, backend)');
const selectPosition = lease.indexOf('select_schema_jobs_sql(backend)');
if (
  lockPosition < 0 ||
  verifyPosition < 0 ||
  selectPosition < 0 ||
  !(lockPosition < verifyPosition && verifyPosition < selectPosition)
) {
  fail('schema advisory lock, persisted-schema verification, and job selection must remain ordered');
}

for (const forbidden of [
  'rustok_product',
  'rustok_content',
  'rustok_flex',
  'rustok_pricing',
  'rustok_inventory',
  'CREATE INDEX CONCURRENTLY',
]) {
  if (lease.includes(forbidden)) fail(`${leasePath} contains forbidden marker ${forbidden}`);
}

requireMarkers('crates/rustok-index/src/infrastructure/postgres/schema_lease_tests.rs', [
  'acquire_excludes_other_workers_and_completion_is_terminal',
  'expired_lease_is_reclaimed_with_attempt_fencing',
  'schema_registration_and_request_identity_fail_closed',
  'SchemaLeaseAcquireOutcome::Busy',
  'SchemaLeaseAcquireOutcome::AlreadyApplied',
  'SchemaLeaseError::LeaseLost',
]);
requireMarkers('crates/rustok-index/src/infrastructure/postgres/mod.rs', [
  'mod schema_lease;',
  'mod schema_lease_tests;',
  'PostgresSchemaLeaseStore',
  'SchemaApplicationLeaseRequest',
]);
requireMarkers('crates/rustok-index/src/lib.rs', [
  'PostgresSchemaLeaseStore',
  'SchemaApplicationLease',
  'SchemaLeaseAcquireOutcome',
]);
requireMarkers('crates/rustok-index/docs/implementation-plan.md', [
  '- [x] Add locking/leases for schema application.',
  'M3 schema-application leases: `complete`',
]);

console.log('[verify-index-schema-leases] OK');

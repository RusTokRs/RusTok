#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const read = (relative) => fs.readFileSync(path.join(root, relative), 'utf8');
const fail = (message) => {
  console.error(`[verify-index-drift-finding-writer] ${message}`);
  process.exit(1);
};
const requireMarkers = (relative, markers) => {
  const source = read(relative);
  for (const marker of markers) {
    if (!source.includes(marker)) fail(`${relative} is missing ${marker}`);
  }
  return source;
};

const writerPath =
  'crates/rustok-index/src/infrastructure/postgres/drift_finding_writer.rs';
const writer = requireMarkers(writerPath, [
  'const FINDING_KEY_CONTRACT: &[u8] = b"index_drift_finding_key_v1";',
  'const FINDING_DETAILS_CONTRACT: &str = "index_drift_digest_finding_v1";',
  'pub struct IndexDriftDigestFindingRequest',
  'pub enum IndexDriftFindingWriteOutcome',
  'pub enum IndexDriftFindingWriteError',
  'pub struct PostgresIndexDriftFindingWriter',
  'pub async fn record_digest_mismatch(',
  'IndexDriftFindingWriteOutcome::Created',
  'IndexDriftFindingWriteOutcome::Refreshed',
  'IndexDriftFindingWriteOutcome::Reopened',
  'IndexDriftFindingWriteOutcome::Suppressed',
  'writer_creates_refreshes_reopens_and_preserves_ignored_suppression',
  'writer_rejects_stored_identity_drift_for_the_same_key',
]);
const production = writer.split('\n#[cfg(test)]')[0];

for (const forbidden of [
  'tokio::spawn',
  'spawn_blocking',
  'tokio::time::sleep',
  'std::thread::sleep',
  'Router::new',
  'async_graphql',
  'ModuleWorkScheduler',
  'PostgresMutationStore',
  '.scan(',
  '.load(',
  'repair_finding',
  'resolve_finding',
  'ignore_finding',
]) {
  if (production.includes(forbidden)) {
    fail(`${writerPath} production boundary contains forbidden marker ${forbidden}`);
  }
}
if (production.includes('Storage(String)') || production.includes('Storage(#[source]')) {
  fail(`${writerPath} storage errors must not retain database details`);
}

const constructorStart = production.indexOf('    pub fn new(');
const constructorEnd = production.indexOf('\n    pub fn tenant_id(', constructorStart);
const constructor = production.slice(constructorStart, constructorEnd);
for (const marker of [
  'if tenant_id.is_nil()',
  'validate_check_name(&check_name)?;',
  'PersistedFindingScope::from_scope(&scope)?;',
  'validate_digest(&expected_digest, "expected digest")?;',
  'validate_digest(&actual_digest, "actual digest")?;',
  'if expected_digest == actual_digest',
  'derive_finding_key(tenant_id, &check_name, &scope)',
]) {
  if (!constructor.includes(marker)) fail(`${writerPath} request validation is missing ${marker}`);
}

const keyStart = production.indexOf('fn derive_finding_key(');
const hashSchemaStart = production.indexOf('\nfn hash_schema(', keyStart);
if (keyStart < 0 || hashSchemaStart <= keyStart) {
  fail(`${writerPath} deterministic finding key segment is missing`);
}
const key = production.slice(keyStart, hashSchemaStart);
for (const marker of [
  'FINDING_KEY_CONTRACT',
  'tenant_id.as_bytes()',
  'check_name.as_bytes()',
  'b"global"',
  'b"schema"',
  'b"entity"',
  'hash_schema(&mut hasher, schema)',
  'entity_id.as_bytes()',
  'locale.as_str().as_bytes()',
]) {
  if (!key.includes(marker)) fail(`${writerPath} deterministic key is missing ${marker}`);
}
for (const forbidden of ['expected_digest', 'actual_digest', 'severity']) {
  if (key.includes(forbidden)) {
    fail(`${writerPath} deterministic key must not include changing field ${forbidden}`);
  }
}

const transactionStart = production.indexOf('    async fn record_in_transaction(');
const scopeStructStart = production.indexOf('\n#[derive(Debug, Clone, PartialEq, Eq)]', transactionStart);
const transaction = production.slice(transactionStart, scopeStructStart);
const backend = transaction.indexOf('ensure_supported_backend(backend)?;');
const lock = transaction.indexOf('lock_finding_key(transaction, request, backend).await?;');
const load = transaction.indexOf('load_existing_finding(transaction, request, backend).await?');
const insert = transaction.indexOf('insert_finding_sql(backend)');
if (backend < 0 || lock <= backend || load <= lock || insert <= load) {
  fail(`${writerPath} must validate backend, lock key, load existing row, then insert`);
}

const lockStart = production.indexOf('async fn lock_finding_key(');
const loadStart = production.indexOf('async fn load_existing_finding(', lockStart);
const lockBody = production.slice(lockStart, loadStart);
for (const marker of [
  'index-drift-finding\\u{1f}',
  'request.tenant_id()',
  'request.finding_key()',
  'SELECT pg_advisory_xact_lock(hashtextextended($1, 0))',
]) {
  if (!lockBody.includes(marker)) fail(`${writerPath} lock contract is missing ${marker}`);
}

const refreshStart = production.indexOf('async fn refresh_existing_finding(');
const lockFunctionStart = production.indexOf('async fn lock_finding_key(', refreshStart);
const refresh = production.slice(refreshStart, lockFunctionStart);
for (const marker of [
  'existing.check_name != request.check_name()',
  'existing.scope != *expected_scope',
  '"open" =>',
  'refresh_open_finding_sql(backend)',
  '"resolved" =>',
  'reopen_resolved_finding_sql(backend)',
  '"ignored" =>',
  'refresh_ignored_finding_sql(backend)',
  'IndexDriftFindingWriteOutcome::Suppressed',
  'if updated.rows_affected() != 1',
]) {
  if (!refresh.includes(marker)) fail(`${writerPath} lifecycle contract is missing ${marker}`);
}

const detailsStart = production.indexOf('fn finding_details()');
const insertValuesStart = production.indexOf('fn insert_values(', detailsStart);
const details = production.slice(detailsStart, insertValuesStart);
if (!details.includes('json!({ "contract": FINDING_DETAILS_CONTRACT })')) {
  fail(`${writerPath} must generate the fixed one-field details contract`);
}
for (const forbidden of [
  'tenant_id', 'finding_id', 'finding_key', 'check_name', 'severity', 'scope_kind',
  'expected_digest', 'actual_digest', 'source', 'payload', 'actor', 'reason', 'sql',
]) {
  if (details.includes(forbidden)) {
    fail(`${writerPath} fixed details contain forbidden field ${forbidden}`);
  }
}

for (const [name, markers] of [
  ['insert_finding_sql', [
    'INSERT INTO index_consistency_findings',
    "'open'",
    'ON CONFLICT (tenant_id, finding_key) DO NOTHING',
  ]],
  ['select_existing_finding_sql', [
    'SELECT finding_id, check_name, state, scope_kind',
    'WHERE tenant_id = $1 AND finding_key = $2 FOR UPDATE',
  ]],
  ['refresh_open_finding_sql', [
    "state = 'open'",
    'last_detected_at = CURRENT_TIMESTAMP',
  ]],
  ['reopen_resolved_finding_sql', [
    "state = 'open'",
    'closed_at = NULL',
    "state = 'resolved'",
  ]],
  ['refresh_ignored_finding_sql', [
    'last_detected_at = CURRENT_TIMESTAMP',
    "state = 'ignored'",
  ]],
]) {
  const start = production.indexOf(`fn ${name}(`);
  if (start < 0) fail(`${writerPath} is missing ${name}`);
  const next = production.indexOf('\nfn ', start + 4);
  const body = production.slice(start, next < 0 ? production.length : next);
  for (const marker of markers) {
    if (!body.includes(marker)) fail(`${writerPath} ${name} is missing ${marker}`);
  }
}

requireMarkers('crates/rustok-index/src/infrastructure/postgres/mod.rs', [
  'mod drift_finding_writer;',
  'pub use drift_finding_writer::{',
  'IndexDriftDigestFindingRequest, IndexDriftFindingWriteError, IndexDriftFindingWriteOutcome,',
  'PostgresIndexDriftFindingWriter,',
]);

const serverPath = 'apps/server/src/services/index_reconciliation_operator.rs';
const server = read(serverPath);
for (const premature of [
  'PostgresIndexDriftFindingWriter',
  'IndexDriftDigestFindingRequest',
  'record_digest_mismatch(',
]) {
  if (server.includes(premature)) {
    fail(`${serverPath} prematurely publishes writer marker ${premature}`);
  }
}

requireMarkers('crates/rustok-index/docs/m6-drift-finding-writer.md', [
  'Status: `source_complete_producer_and_repair_pending`.',
  '`PostgresIndexDriftFindingWriter`',
  'Expected and actual digests are deliberately excluded from the key.',
  'existing `ignored` row -> `Suppressed`',
  'Callers cannot add arbitrary details.',
  'The writer is not composed into this server runtime.',
  'authoritative source/index digest computation and producer composition',
  'The canonical roadmap item `Add drift diagnosis, targeted repair commands, and admitted repair evidence` remains open.',
  'maintainer-run',
]);
requireMarkers('crates/rustok-index/docs/m6-drift-finding-inspection.md', [
  'The separate `PostgresIndexDriftFindingWriter` can now persist',
  'The writer is not composed into this server runtime.',
  'authoritative source/index digest computation and producer composition',
]);
requireMarkers('crates/rustok-index/docs/README.md', [
  '[M6 Drift Finding Inspection](./m6-drift-finding-inspection.md)',
  '[M6 Drift Digest Finding Writer](./m6-drift-finding-writer.md)',
]);
requireMarkers('crates/rustok-index/docs/implementation-plan.md', [
  '- [ ] Add drift diagnosis, targeted repair commands, and admitted repair evidence.',
]);
requireMarkers('scripts/verify/verify-index-query-contract.mjs', [
  "'verify-index-drift-finding-inspection.mjs'",
  "'verify-index-drift-finding-writer.mjs'",
]);

console.log('[verify-index-drift-finding-writer] OK');

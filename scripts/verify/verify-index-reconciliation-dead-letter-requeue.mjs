#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const read = (relative) => fs.readFileSync(path.join(root, relative), 'utf8');
const fail = (message) => {
  console.error(`[verify-index-reconciliation-dead-letter-requeue] ${message}`);
  process.exit(1);
};
const requireMarkers = (relative, markers) => {
  const source = read(relative);
  for (const marker of markers) {
    if (!source.includes(marker)) fail(`${relative} is missing ${marker}`);
  }
  return source;
};

const migrationModulePath = 'crates/rustok-index/src/migrations/mod.rs';
const migrationPath =
  'crates/rustok-index/src/migrations/m20260803_000004_create_index_reconciliation_recovery.rs';
const recoveryPath =
  'crates/rustok-index/src/infrastructure/postgres/source_reconciliation_recovery.rs';
const postgresPath = 'crates/rustok-index/src/infrastructure/postgres/mod.rs';
const runnerPath =
  'crates/rustok-index/src/infrastructure/postgres/source_reconciliation_runner.rs';
const serverPath = 'apps/server/src/services/index_reconciliation_operator.rs';
const serverDocsPath = 'apps/server/docs/index-reconciliation-operator-runtime.md';
const docsPath = 'crates/rustok-index/docs/m6-reconciliation-dead-letter-requeue.md';
const docsIndexPath = 'crates/rustok-index/docs/README.md';
const planPath = 'crates/rustok-index/docs/implementation-plan.md';
const aggregatePath = 'scripts/verify/verify-index-query-contract.mjs';

requireMarkers(migrationModulePath, [
  'mod m20260803_000004_create_index_reconciliation_recovery;',
  'm20260803_000004_create_index_reconciliation_recovery::Migration',
  '"m20260803_000004_create_index_reconciliation_recovery"',
  'vec!["m20260727_000003_create_index_operations"]',
]);

const migration = requireMarkers(migrationPath, [
  'ALTER TABLE index_jobs',
  'ADD COLUMN retry_epoch INTEGER NOT NULL DEFAULT 0',
  'CREATE TABLE index_reconciliation_recovery_audits',
  'actor_id UUID NOT NULL',
  "CHECK (action = 'requeue')",
  'reason VARCHAR(512) NOT NULL',
  'prior_attempt_count INTEGER NOT NULL',
  'retry_epoch INTEGER NOT NULL',
  'UNIQUE (tenant_id, job_id, retry_epoch)',
  'index_reconciliation_recovery_audits_immutable_update',
  'index_reconciliation_recovery_audits_immutable_delete',
  "RAISE EXCEPTION 'Index reconciliation recovery audits are append-only'",
  "SELECT RAISE(ABORT, 'Index reconciliation recovery audits are append-only')",
]);
for (const forbidden of ['ON UPDATE CASCADE', 'ON DELETE CASCADE', '.if_not_exists()']) {
  if (migration.includes(forbidden)) {
    fail(`${migrationPath} contains forbidden mutability/drift marker ${forbidden}`);
  }
}

const recovery = requireMarkers(recoveryPath, [
  'const RECONCILIATION_CURSOR_CONTRACT: &str = "index_reconciliation_cursor_v1";',
  'const RECONCILIATION_RECOVERY_ACTION: &str = "requeue";',
  'const MAX_RECOVERY_REASON_BYTES: usize = 512;',
  'pub struct IndexReconciliationRequeueRequest',
  'pub enum IndexReconciliationRequeueOutcome',
  'pub struct PostgresIndexReconciliationRecoveryStore',
  'pub async fn requeue_failed(',
  'select_recovery_scope(transaction, request, backend, false).await?',
  'lock_reconciliation_scope(transaction, request.tenant_id, &scope, backend).await?;',
  'select_recovery_scope(transaction, request, backend, true).await?',
  'if locked.state != "failed"',
  '.checked_add(1)',
  'update_failed_job_sql(backend)',
  'insert_audit_sql(backend)',
  'transaction.commit()',
  'transaction.rollback()',
  'requeue_resets_same_job_and_appends_immutable_audit',
  'requeue_is_one_shot_for_each_failed_epoch',
  'request_rejects_nil_identity_and_unbounded_reason',
]);

const transactionStart = recovery.indexOf('async fn requeue_in_transaction(');
const transactionEnd = recovery.indexOf('\nasync fn select_recovery_scope(', transactionStart);
if (transactionStart < 0 || transactionEnd <= transactionStart) {
  fail(`${recoveryPath} bounded recovery transaction is missing`);
}
const transaction = recovery.slice(transactionStart, transactionEnd);
const firstSelect = transaction.indexOf(
  'select_recovery_scope(transaction, request, backend, false)',
);
const scopeLock = transaction.indexOf('lock_reconciliation_scope(');
const lockedSelect = transaction.indexOf(
  'select_recovery_scope(transaction, request, backend, true)',
);
const failedCheck = transaction.indexOf('if locked.state != "failed"');
const jobUpdate = transaction.indexOf('update_failed_job_sql(backend)');
const auditInsert = transaction.indexOf('insert_audit_sql(backend)');
if (
  firstSelect < 0
  || scopeLock <= firstSelect
  || lockedSelect <= scopeLock
  || failedCheck <= lockedSelect
  || jobUpdate <= failedCheck
  || auditInsert <= jobUpdate
) {
  fail(`${recoveryPath} must resolve, lock, re-read, validate, reset, then audit`);
}

const lockStart = recovery.indexOf('async fn lock_reconciliation_scope(');
const lockEnd = recovery.indexOf('\nfn initial_cursor()', lockStart);
const lock = recovery.slice(lockStart, lockEnd);
for (const marker of [
  '"reconcile\\u{1f}{}\\u{1f}{}\\u{1f}{}\\u{1f}{}"',
  'tenant_id, scope.module_name, scope.entity_name, scope.schema_version',
  'SELECT pg_advisory_xact_lock(hashtextextended($1, 0))',
]) {
  if (!lock.includes(marker)) fail(`${recoveryPath} scope lock is missing ${marker}`);
}

const runner = requireMarkers(runnerPath, [
  '"reconcile\\u{1f}{}\\u{1f}{}\\u{1f}{}\\u{1f}{}"',
  'SELECT pg_advisory_xact_lock(hashtextextended($1, 0))',
  'IndexReconciliationRunError::DeadLettered',
]);
if (!runner.includes("state IN ('pending', 'running', 'succeeded', 'failed')")) {
  fail(`${runnerPath} failed-scope admission must remain active`);
}

const updateStart = recovery.indexOf('fn update_failed_job_sql(');
const updateEnd = recovery.indexOf('\nfn insert_audit_sql(', updateStart);
const update = recovery.slice(updateStart, updateEnd);
for (const marker of [
  "state = 'pending'",
  'cursor = {prefix}6',
  'attempt_count = 0',
  'available_at = CURRENT_TIMESTAMP',
  'lease_owner = NULL',
  'lease_expires_at = NULL',
  'heartbeat_at = NULL',
  'cancel_requested = FALSE',
  'last_error_code = NULL',
  'last_error_details = NULL',
  'completed_at = NULL',
  'retry_epoch = {prefix}5',
  "kind = 'reconcile'",
  "state = 'failed'",
  'attempt_count = {prefix}3',
  'retry_epoch = {prefix}4',
]) {
  if (!update.includes(marker)) fail(`${recoveryPath} failed-job reset is missing ${marker}`);
}

const production = recovery.split('\n#[cfg(test)]')[0];
for (const forbidden of [
  'INSERT INTO index_jobs',
  '.sources.scan(',
  '.sources.load(',
  'tokio::spawn',
  'spawn_blocking',
  'tokio::time::sleep',
  'Router::new',
  'async_graphql',
  'poll',
  'scheduler',
]) {
  if (production.includes(forbidden)) {
    fail(`${recoveryPath} production boundary contains forbidden marker ${forbidden}`);
  }
}
if (production.includes('Storage(String)') || production.includes('Storage(#[source]')) {
  fail(`${recoveryPath} storage errors must not retain database details`);
}

requireMarkers(postgresPath, [
  'mod source_reconciliation_recovery;',
  'pub use source_reconciliation_recovery::{',
  'IndexReconciliationRecoveryError, IndexReconciliationRequeueOutcome,',
  'IndexReconciliationRequeueRequest, PostgresIndexReconciliationRecoveryStore,',
  'PostgresIndexReconciliationDeadLetterInspector,',
  'PostgresIndexReconciliationRunner,',
]);

const server = requireMarkers(serverPath, [
  'recovery: rustok_index::infrastructure::postgres::PostgresIndexReconciliationRecoveryStore,',
  'Recovery(#[from] rustok_index::infrastructure::postgres::IndexReconciliationRecoveryError),',
  'pub async fn requeue_dead_letter(',
  'context.authorize_for(context.tenant_id())?;',
  'IndexReconciliationRequeueRequest::new(',
  'context.tenant_id(),',
  'context.actor_id(),',
  '.requeue_failed(request)',
  'PostgresIndexReconciliationRecoveryStore::new(',
  'dead_letter_requeue_authorizes_before_request_validation',
]);
const requeueStart = server.indexOf('    pub async fn requeue_dead_letter(');
const requeueEnd = server.indexOf('\n}\n\nimpl fmt::Debug', requeueStart);
if (requeueStart < 0 || requeueEnd <= requeueStart) {
  fail(`${serverPath} guarded requeue method is malformed`);
}
const requeue = server.slice(requeueStart, requeueEnd);
for (const forbidden of ['tenant_id: Uuid', 'actor_id: Uuid']) {
  if (requeue.includes(forbidden)) {
    fail(`${serverPath} guarded requeue accepts caller-selected ${forbidden}`);
  }
}
const authorization = requeue.indexOf('context.authorize_for(context.tenant_id())?;');
const request = requeue.indexOf('IndexReconciliationRequeueRequest::new(');
const tenant = requeue.indexOf('context.tenant_id(),', request);
const actor = requeue.indexOf('context.actor_id(),', tenant);
const delegation = requeue.indexOf('.requeue_failed(request)');
if (
  authorization < 0
  || request <= authorization
  || tenant <= request
  || actor <= tenant
  || delegation <= actor
) {
  fail(`${serverPath} must authorize, bind context tenant/actor, then delegate`);
}
for (const forbidden of ['SELECT ', 'INSERT ', 'UPDATE ', 'DELETE ', "state = 'pending'"]) {
  if (requeue.includes(forbidden)) {
    fail(`${serverPath} guarded requeue duplicates engine detail ${forbidden}`);
  }
}

requireMarkers(docsPath, [
  'Status: `source_complete_authorized_server_composition_transport_pending`.',
  'same PostgreSQL transaction-scoped advisory lock used by normal reconciliation admission',
  'preserves the existing job UUID',
  'increments `retry_epoch`',
  'appends an immutable actor/reason audit record',
  'The audit insert and job reset commit or roll back together.',
  '## Authorized server composition',
  'accepts no tenant or actor argument',
  'Authorization occurs before job/reason validation',
  'GraphQL, HTTP, CLI, MCP, native admin',
  'automatic retry, backoff, exhaustion, scheduling, and graceful shutdown',
  'maintainer-run',
]);
requireMarkers(serverDocsPath, [
  'Status: `source_complete_transport_and_scheduling_pending`.',
  'tenant/actor-bound `requeue_dead_letter(context, job_id, reason)`',
  'caller-selected tenant or actor identity for recovery',
  'The same-job failed-to-pending reset',
]);
requireMarkers(docsIndexPath, [
  '[M6 Reconciliation Dead-letter Inspection](./m6-reconciliation-dead-letter-inspection.md)',
  '[M6 Reconciliation Dead-letter Requeue](./m6-reconciliation-dead-letter-requeue.md)',
]);
requireMarkers(planPath, [
  '- [ ] Add bounded retry/backoff, dead-letter state, and global scheduling ownership.',
  '- [ ] Add drift diagnosis, targeted repair commands, and admitted repair evidence.',
]);
requireMarkers(aggregatePath, [
  "'verify-index-reconciliation-dead-letter-inspection.mjs'",
  "'verify-index-reconciliation-dead-letter-requeue.mjs'",
  "'verify-index-server-reconciliation-guard.mjs'",
]);

console.log('[verify-index-reconciliation-dead-letter-requeue] OK');

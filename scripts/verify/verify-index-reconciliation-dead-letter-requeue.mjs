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

const migrationPath =
  'crates/rustok-index/src/migrations/m20260803_000004_create_index_reconciliation_recovery.rs';
const recoveryPath =
  'crates/rustok-index/src/infrastructure/postgres/source_reconciliation_recovery.rs';
const operatorPath = 'apps/server/src/services/index_reconciliation_operator.rs';
const schedulerPath =
  'crates/rustok-index/src/infrastructure/postgres/source_reconciliation_scheduler.rs';
const docsPath =
  'crates/rustok-index/docs/m6-reconciliation-dead-letter-requeue.md';
const serverDocsPath = 'apps/server/docs/index-reconciliation-operator-runtime.md';
const planPath = 'crates/rustok-index/docs/implementation-plan.md';

requireMarkers(migrationPath, [
  'ADD COLUMN retry_epoch INTEGER NOT NULL DEFAULT 0',
  'CREATE TABLE index_reconciliation_recovery_audits',
  'actor_id UUID NOT NULL',
  'reason VARCHAR(512) NOT NULL',
  'UNIQUE (tenant_id, job_id, retry_epoch)',
  'index_reconciliation_recovery_audits_immutable_update',
  'index_reconciliation_recovery_audits_immutable_delete',
]);
const recovery = requireMarkers(recoveryPath, [
  'pub struct IndexReconciliationRequeueRequest',
  'pub struct PostgresIndexReconciliationRecoveryStore',
  'pub async fn requeue_failed(',
  'lock_reconciliation_scope(',
  'if locked.state != "failed"',
  'update_failed_job_sql(backend)',
  'insert_audit_sql(backend)',
  "state = 'pending'",
  'attempt_count = 0',
  'retry_epoch = {prefix}5',
  'transaction.commit()',
  'transaction.rollback()',
]);
const production = recovery.split('\n#[cfg(test)]')[0];
for (const forbidden of [
  'INSERT INTO index_jobs', '.sources.scan(', 'tokio::spawn', 'tokio::time::sleep',
  'Router::new', 'async_graphql',
]) {
  if (production.includes(forbidden)) fail(`${recoveryPath} contains ${forbidden}`);
}

requireMarkers(operatorPath, [
  'pub async fn requeue_dead_letter(',
  'context.authorize_for(context.tenant_id())?;',
  'IndexReconciliationRequeueRequest::new(',
  'context.tenant_id(),',
  'context.actor_id(),',
  '.requeue_failed(request)',
]);
requireMarkers(schedulerPath, [
  "state = 'pending' AND available_at <= CURRENT_TIMESTAMP",
  '.runner',
  '.run(request)',
]);
requireMarkers(docsPath, [
  'Status: `source_complete_authorized_server_composition_transport_pending`.',
  'preserves the existing job UUID',
  'The audit insert and job reset commit or roll back together',
  'accepts no tenant or actor argument',
  'The module-owned host scheduler is additive and unchanged by manual recovery',
  'Automatic retry creates no recovery audit',
  'maintainer-run',
]);
requireMarkers(serverDocsPath, [
  'Status: `sealed_source_page_graphql_source_complete_owner_execution_pending`.',
  'The operator runtime does not expose or own that scheduler',
]);
requireMarkers(planPath, [
  '- [ ] Add bounded retry/backoff, dead-letter state, and global scheduling ownership.',
  '- [ ] Add drift diagnosis, targeted repair commands, and admitted repair evidence.',
]);
requireMarkers('scripts/verify/verify-index-query-contract.mjs', [
  "'verify-index-reconciliation-dead-letter-requeue.mjs'",
  "'verify-index-reconciliation-host-scheduler.mjs'",
]);

console.log('[verify-index-reconciliation-dead-letter-requeue] OK');

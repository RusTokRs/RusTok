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
  'crates/modules/rustok-index/src/migrations/m20260803_000004_create_index_reconciliation_recovery.rs';
const recoveryPath =
  'crates/modules/rustok-index/src/infrastructure/postgres/source_job_recovery.rs';
const operatorPath = 'apps/server/src/services/index_reconciliation_operator.rs';
const graphqlPath = 'apps/server/src/graphql/index_replay.rs';
const nativePath = 'crates/modules/rustok-index/admin/src/transport/native_server_adapter.rs';
const schedulerPath =
  'crates/modules/rustok-index/src/infrastructure/postgres/source_reconciliation_scheduler.rs';
const docsPath =
  'crates/modules/rustok-index/docs/m6-reconciliation-dead-letter-requeue.md';
const serverDocsPath = 'apps/server/docs/index-reconciliation-operator-runtime.md';
const planPath = 'crates/modules/rustok-index/docs/implementation-plan.md';

requireMarkers(migrationPath, [
  'ADD COLUMN retry_epoch INTEGER NOT NULL DEFAULT 0',
  'CREATE TABLE index_job_recovery_audits',
  'actor_id UUID NOT NULL',
  'reason VARCHAR(512) NOT NULL',
  'UNIQUE (tenant_id, job_id, retry_epoch)',
  'index_job_recovery_audits_immutable_update',
  'index_job_recovery_audits_immutable_delete',
  'index_job_recovery_audits_immutable_truncate',
  'index_job_recovery_audits_no_replacement',
]);
const recovery = requireMarkers(recoveryPath, [
  'pub struct IndexJobRequeueRequest',
  'pub(super) struct PostgresIndexJobRecoveryStore',
  'async fn requeue_failed(',
  'lock_reconciliation_scope(',
  'if scope.state != "failed" || locked.state != "failed"',
  'update_failed_job_sql(backend, kind)',
  'insert_audit_sql(backend)',
  "state = 'pending'",
  'failure_count = 0',
  'retry_epoch = {prefix}5',
  '.commit()',
  '.rollback()',
  'locked.attempt_count != scope.attempt_count || locked.retry_epoch != scope.retry_epoch',
  'if current != Some(request.job_id)',
]);
const transition = recovery.slice(recovery.indexOf('fn update_failed_job_sql('), recovery.indexOf('fn insert_audit_sql('));
if (/\brequest\s*=|\battempt_count\s*=\s*0/.test(transition)) {
  fail('requeuing must not replace the original job request or reset its lease generation');
}
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
  'IndexJobRequeueRequest::new(',
  'context.tenant_id(),',
  'context.actor_id(),',
  '.requeue_failed(request)',
]);
const graphql = read(graphqlPath);
const input = graphql.slice(graphql.indexOf('pub struct IndexJobRequeueInput {'), graphql.indexOf('pub enum IndexJobRequeueStatus'));
if (!/pub struct IndexJobRequeueInput \{\s*pub job_id: Uuid,\s*pub reason: String,\s*\}/u.test(input)) {
  fail('requeue input must not admit tenant, actor, attempt or cursor overrides');
}
for (const [field, dispatch] of [
  ['requeue_index_replay', '.requeue_failed(operator, input.job_id, input.reason)'],
  ['requeue_index_reconciliation', '.requeue_dead_letter(operator, input.job_id, input.reason)'],
]) {
  const start = graphql.indexOf(`async fn ${field}(`);
  const end = graphql.indexOf('\n    }', start);
  const body = graphql.slice(start, end);
  const scope = body.indexOf('request_scope(ctx)?');
  const permission = body.indexOf('authorize(tenant_id, actor_id)', scope);
  const call = body.indexOf(dispatch, permission);
  if (scope < 0 || permission <= scope || call <= permission) {
    fail(`${field} must authorize the request before dispatching the exact job kind`);
  }
}
requireMarkers(nativePath, [
  'let (authority, runtime, _) = operator_context().await?;',
  'let outcome = match input.kind {',
  'RetryJobKind::Replay =>',
  'RetryJobKind::Reconciliation =>',
  '.requeue_operator_replay(authority, job_id, reason)',
  '.requeue_operator_reconciliation(authority, job_id, reason)',
  'IndexJobRequeueOutcome::NotFound => (RetryActionStatus::NotFound, None)',
]);

requireMarkers(schedulerPath, [
  "available_at <= CURRENT_TIMESTAMP",
  '.runner',
  '.run(request)',
]);
requireMarkers(docsPath, [
  'Status: source complete; native and GraphQL execution verification pending.',
  'resets `failure_count` to zero without resetting the lease generation',
  '`requeueIndexReconciliation` binds tenant and actor in the resolver.',
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

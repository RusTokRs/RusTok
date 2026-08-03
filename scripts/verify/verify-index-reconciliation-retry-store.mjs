#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const read = (relative) => fs.readFileSync(path.join(root, relative), 'utf8');
const fail = (message) => {
  console.error(`[verify-index-reconciliation-retry-store] ${message}`);
  process.exit(1);
};
const requireMarkers = (relative, markers) => {
  const source = read(relative);
  for (const marker of markers) {
    if (!source.includes(marker)) fail(`${relative} is missing ${marker}`);
  }
  return source;
};

const retryPath =
  'crates/rustok-index/src/infrastructure/postgres/source_reconciliation_retry.rs';
const postgresPath = 'crates/rustok-index/src/infrastructure/postgres/mod.rs';
const libPath = 'crates/rustok-index/src/lib.rs';
const runnerPath =
  'crates/rustok-index/src/infrastructure/postgres/source_reconciliation_runner.rs';
const docsPath =
  'crates/rustok-index/docs/m6-reconciliation-retry-transition-store.md';
const docsIndexPath = 'crates/rustok-index/docs/README.md';
const planPath = 'crates/rustok-index/docs/implementation-plan.md';
const aggregatePath = 'scripts/verify/verify-index-query-contract.mjs';

const retry = requireMarkers(retryPath, [
  'const MAX_RECONCILIATION_ATTEMPTS: u32 = 100;',
  'const MAX_BACKOFF_SECONDS: u64 = 86_400;',
  'const MAX_FAILURE_CODE_BYTES: usize = 128;',
  'const MAX_WORKER_ID_BYTES: usize = 191;',
  'const RECONCILIATION_FAILURE_CONTRACT: &str = "index_reconciliation_run_failure_v1";',
  'const RECONCILIATION_PAGE_FAILURE_CODE: &str = "index.reconciliation_page_failed";',
  'pub struct IndexReconciliationRetryLease',
  'pub enum IndexReconciliationRetryFailureKind',
  'pub struct IndexReconciliationRetryFailure',
  'pub struct IndexReconciliationRetryPolicy',
  'pub enum IndexReconciliationRetryDisposition',
  'pub enum IndexReconciliationRetryError',
  'pub struct PostgresIndexReconciliationRetryStore',
  'pub async fn record_failure(',
  'IndexReconciliationRetryDisposition::RetryScheduled',
  'IndexReconciliationRetryDisposition::TerminalPermanent',
  'IndexReconciliationRetryDisposition::TerminalExhausted',
  'default reconciliation retry policy must be valid',
  'Self::new(5, Duration::from_secs(5), Duration::from_secs(300))',
  'default_policy_uses_bounded_exponential_backoff',
  'diagnostics_keep_the_existing_inspection_contract',
]);

const production = retry.split('\n#[cfg(test)]')[0];
for (const forbidden of [
  'INSERT INTO index_jobs',
  'DELETE FROM index_jobs',
  'tokio::spawn',
  'spawn_blocking',
  'tokio::time::sleep',
  'std::thread::sleep',
  'loop {',
  'Router::new',
  'async_graphql',
  'reqwest',
]) {
  if (production.includes(forbidden)) {
    fail(`${retryPath} production boundary contains forbidden marker ${forbidden}`);
  }
}
if (production.includes('Storage(String)') || production.includes('Storage(#[source]')) {
  fail(`${retryPath} storage errors must not retain database details`);
}

const policyStart = production.indexOf('impl IndexReconciliationRetryPolicy');
const policyEnd = production.indexOf('\nimpl Default for IndexReconciliationRetryPolicy', policyStart);
if (policyStart < 0 || policyEnd <= policyStart) {
  fail(`${retryPath} bounded policy implementation is missing`);
}
const policy = production.slice(policyStart, policyEnd);
for (const marker of [
  '1..=MAX_RECONCILIATION_ATTEMPTS',
  'validate_backoff(base_backoff)?',
  'validate_backoff(max_backoff)?',
  'base_backoff_seconds > max_backoff_seconds',
  'failure_kind == IndexReconciliationRetryFailureKind::Permanent',
  'attempt_count >= self.max_attempts',
  'saturating_mul(multiplier)',
  '.min(self.max_backoff_seconds)',
  '.checked_add(1)',
]) {
  if (!policy.includes(marker)) fail(`${retryPath} policy is missing ${marker}`);
}

const recordStart = production.indexOf('    pub async fn record_failure(');
const recordEnd = production.indexOf('\n}\n\nasync fn terminalize_failure(', recordStart);
if (recordStart < 0 || recordEnd <= recordStart) {
  fail(`${retryPath} record_failure boundary is malformed`);
}
const record = production.slice(recordStart, recordEnd);
const evaluate = record.indexOf('.evaluate(lease.attempt_count(), failure.kind())?;');
const backend = record.indexOf('let backend = self.db.get_database_backend();');
const details = record.indexOf('let details = failure_details(failure);');
const transition = record.indexOf('let rows_affected = match disposition');
const fence = record.indexOf('if rows_affected != 1');
if (
  evaluate < 0
  || backend <= evaluate
  || details <= backend
  || transition <= details
  || fence <= transition
) {
  fail(`${retryPath} must evaluate policy, validate backend, build diagnostics, transition, then fence`);
}

const scheduleStart = production.indexOf('fn schedule_retry_sql(');
const terminalStart = production.indexOf('fn terminal_failure_sql(', scheduleStart);
const schedule = production.slice(scheduleStart, terminalStart);
for (const marker of [
  "state = 'pending'",
  "kind = 'reconcile'",
  "state = 'running'",
  'available_at = {available_at}',
  'lease_owner = NULL',
  'lease_expires_at = NULL',
  'completed_at = NULL',
  'last_error_code = {prefix}6',
  'last_error_details = {prefix}7',
  'lease_owner = {prefix}3',
  'attempt_count = {prefix}4',
  'lease_expires_at > CURRENT_TIMESTAMP',
  'cancel_requested = FALSE',
]) {
  if (!schedule.includes(marker)) fail(`${retryPath} retry SQL is missing ${marker}`);
}

const terminalEnd = production.indexOf('\n}', terminalStart) + 2;
const terminal = production.slice(terminalStart, terminalEnd);
for (const marker of [
  "state = 'failed'",
  "kind = 'reconcile'",
  "state = 'running'",
  'completed_at = CURRENT_TIMESTAMP',
  'last_error_code = {prefix}5',
  'last_error_details = {prefix}6',
  'lease_owner = {prefix}3',
  'attempt_count = {prefix}4',
  'lease_expires_at > CURRENT_TIMESTAMP',
  'cancel_requested = FALSE',
]) {
  if (!terminal.includes(marker)) fail(`${retryPath} terminal SQL is missing ${marker}`);
}

const diagnosticsStart = production.indexOf('fn failure_details(');
const diagnosticsEnd = production.indexOf('\nfn validate_backoff(', diagnosticsStart);
const diagnostics = production.slice(diagnosticsStart, diagnosticsEnd);
for (const marker of [
  '"contract": RECONCILIATION_FAILURE_CONTRACT',
  '"dependency_code": failure.code()',
  '"retryable": failure.kind() == IndexReconciliationRetryFailureKind::Retryable',
]) {
  if (!diagnostics.includes(marker)) fail(`${retryPath} diagnostics are missing ${marker}`);
}
for (const forbidden of [
  'tenant_id',
  'job_id',
  'worker_id',
  'attempt_count',
  'max_attempts',
  'retry_after',
  'next_attempt',
  'retry_epoch',
]) {
  if (diagnostics.includes(forbidden)) {
    fail(`${retryPath} diagnostics contain forbidden field ${forbidden}`);
  }
}

requireMarkers(postgresPath, [
  'mod source_reconciliation_retry;',
  'pub use source_reconciliation_retry::{',
  'IndexReconciliationRetryDisposition, IndexReconciliationRetryError,',
  'IndexReconciliationRetryFailure, IndexReconciliationRetryFailureKind,',
  'IndexReconciliationRetryLease, IndexReconciliationRetryPolicy,',
  'PostgresIndexReconciliationRetryStore,',
  'PostgresIndexReconciliationRecoveryStore,',
  'PostgresIndexReconciliationRunner,',
]);
requireMarkers(libPath, [
  'bounded reconciliation retry transitions',
  'IndexReconciliationRetryDisposition',
  'IndexReconciliationRetryLease',
  'IndexReconciliationRetryPolicy',
  'PostgresIndexReconciliationRetryStore',
]);

const runner = requireMarkers(runnerPath, [
  'pub struct PostgresIndexReconciliationRunner',
  'async fn finish_page_error(',
  'async fn finish_failure(',
  'fn finish_failure_sql(',
  "kind = 'reconcile'",
  "state = 'failed'",
]);
const runnerProduction = runner.split('\n#[cfg(test)]')[0];
for (const premature of [
  'PostgresIndexReconciliationRetryStore',
  'IndexReconciliationRetryPolicy',
  'IndexReconciliationRetryLease',
  'schedule_retry_sql(',
]) {
  if (runnerProduction.includes(premature)) {
    fail(`${runnerPath} prematurely wires retry transition through ${premature}`);
  }
}

requireMarkers(docsPath, [
  'Status: `source_complete_runner_and_scheduler_wiring_pending`.',
  '`PostgresIndexReconciliationRetryStore`',
  'maximum attempts: `5`',
  'delays after attempts 1-4: `5`, `10`, `20`, and `40` seconds',
  'running -> pending',
  'running -> failed',
  'preserve the existing strict `index_reconciliation_run_failure_v1` object',
  'merged dead-letter inspector remains compatible',
  'runner wiring and global scheduling ownership are not part of this slice',
  'maintainer-run',
]);
requireMarkers(docsIndexPath, [
  '[M6 Reconciliation Retry Transition Store](./m6-reconciliation-retry-transition-store.md)',
  '[M6 Reconciliation Dead-letter Admission](./m6-reconciliation-dead-letter-admission.md)',
]);
requireMarkers(planPath, [
  '- [ ] Add bounded retry/backoff, dead-letter state, and global scheduling ownership.',
  '- [ ] Add drift diagnosis, targeted repair commands, and admitted repair evidence.',
]);
requireMarkers(aggregatePath, [
  "'verify-index-reconciliation-retry-store.mjs'",
  "'verify-index-reconciliation-dead-letter-admission.mjs'",
  "'verify-index-reconciliation-dead-letter-requeue.mjs'",
  "'verify-index-server-reconciliation-guard.mjs'",
]);

console.log('[verify-index-reconciliation-retry-store] OK');

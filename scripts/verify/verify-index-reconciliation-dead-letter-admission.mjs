#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const read = (relative) => fs.readFileSync(path.join(root, relative), 'utf8');
const fail = (message) => {
  console.error(`[verify-index-reconciliation-dead-letter-admission] ${message}`);
  process.exit(1);
};
const requireMarkers = (relative, markers) => {
  const source = read(relative);
  for (const marker of markers) {
    if (!source.includes(marker)) fail(`${relative} is missing ${marker}`);
  }
  return source;
};

const runnerPath =
  'crates/rustok-index/src/infrastructure/postgres/source_reconciliation_runner.rs';
const retryPath =
  'crates/rustok-index/src/infrastructure/postgres/source_reconciliation_retry.rs';
const targetPath =
  'crates/rustok-index/tests/source_reconciliation_dead_letter_admission_postgres_test.rs';
const docsPath =
  'crates/rustok-index/docs/m6-reconciliation-dead-letter-admission.md';
const docsIndexPath = 'crates/rustok-index/docs/README.md';
const planPath = 'crates/rustok-index/docs/implementation-plan.md';
const aggregatePath = 'scripts/verify/verify-index-query-contract.mjs';

const runner = requireMarkers(runnerPath, [
  'const MAX_ERROR_CODE_BYTES: usize = 128;',
  'DeadLettered {',
  'job_id: Uuid,',
  'attempt_count: u32,',
  'error_code: Option<String>,',
  'last_error_code: Option<String>,',
  '.try_get("", "last_error_code")',
  'last_error_code is outside the reconciliation error contract',
  'IndexReconciliationRunStatus::FailedPermanent',
  'IndexReconciliationRunStatus::FailedExhausted',
]);

const acquireStart = runner.indexOf('async fn acquire_in_transaction(');
const acquireEnd = runner.indexOf('\nfn validate_stored_request(', acquireStart);
if (acquireStart < 0 || acquireEnd <= acquireStart) {
  fail(`${runnerPath} must retain one bounded acquisition transaction`);
}
const acquire = runner.slice(acquireStart, acquireEnd);
for (const marker of [
  'lock_reconciliation_scope(transaction, request, backend).await?;',
  'verify_schema_registration(transaction, request, backend).await?;',
  '"succeeded" => {',
  '"running" | "pending" if !stored.claimable',
  '"running" | "pending" => {',
  '"failed" => {',
  'return Err(IndexReconciliationRunError::DeadLettered {',
  'error_code: stored.last_error_code,',
]) {
  if (!acquire.includes(marker)) fail(`${runnerPath} acquisition is missing ${marker}`);
}
const succeededBranch = acquire.indexOf('"succeeded" => {');
const busyBranch = acquire.indexOf('"running" | "pending" if !stored.claimable');
const claimableBranch = acquire.indexOf('"running" | "pending" => {');
const failedBranch = acquire.indexOf('"failed" => {');
const insertBranch = acquire.indexOf('let job_id = Uuid::new_v4();');
if (
  succeededBranch < 0
  || busyBranch <= succeededBranch
  || claimableBranch <= busyBranch
  || failedBranch <= claimableBranch
  || insertBranch <= failedBranch
) {
  fail(`${runnerPath} must preserve succeeded, active, failed, then create precedence`);
}
const deadLetterBlock = acquire.slice(failedBranch, acquire.indexOf('state => {', failedBranch));
for (const forbidden of [
  'last_error_details', 'INSERT INTO index_jobs', 'UPDATE index_jobs', 'Uuid::new_v4()', '.scan(',
]) {
  if (deadLetterBlock.includes(forbidden)) {
    fail(`${runnerPath} dead-letter branch contains forbidden marker ${forbidden}`);
  }
}

const selectStart = runner.indexOf('fn select_jobs_sql(');
const selectEnd = runner.indexOf('\nfn insert_job_sql(', selectStart);
const select = runner.slice(selectStart, selectEnd);
for (const marker of [
  'SELECT job_id, state, request, cursor, last_error_code',
  "kind = 'reconcile'",
  "scope_kind = 'schema'",
  "state IN ('pending', 'running', 'succeeded', 'failed')",
  "WHEN 'succeeded' THEN 0 WHEN 'running' THEN 1 WHEN 'pending' THEN 2 ELSE 3",
  'created_at DESC',
]) {
  if (!select.includes(marker)) fail(`${runnerPath} job selection is missing ${marker}`);
}
if (select.includes('last_error_details')) {
  fail(`${runnerPath} ordinary dead-letter admission must not select last_error_details`);
}

const storedStart = runner.indexOf('fn stored_job(');
const storedEnd = runner.indexOf('\nasync fn lock_reconciliation_scope(', storedStart);
const stored = runner.slice(storedStart, storedEnd);
for (const marker of [
  'u32::try_from(attempt_count)',
  'validate_storage_text(code, MAX_ERROR_CODE_BYTES)',
  'last_error_code is outside the reconciliation error contract',
]) {
  if (!stored.includes(marker)) fail(`${runnerPath} stored-job decoder is missing ${marker}`);
}

const retry = requireMarkers(retryPath, [
  'fn terminal_failure_sql(',
  "state = 'failed'",
  'completed_at = CURRENT_TIMESTAMP',
  'last_error_code = {prefix}5',
  'last_error_details = {prefix}6',
  'lease_owner = {prefix}3',
  'attempt_count = {prefix}4',
  'lease_expires_at > CURRENT_TIMESTAMP',
  'cancel_requested = FALSE',
]);
if (runner.includes('fn finish_failure_sql(')) {
  fail(`${runnerPath} must not retain the superseded terminal failure SQL`);
}

const target = requireMarkers(targetPath, [
  'failed_reconciliation_scope_blocks_new_jobs_without_exposing_details',
  'RUSTOK_INDEX_TEST_DATABASE_URL',
  'IndexModule.migrations()',
  'owner_source_permanent_dead_letter',
  'IndexReconciliationRunStatus::FailedPermanent',
  'first_outcome.retry_after(), None',
  'first_outcome.next_attempt(), None',
  'IndexReconciliationRunError::DeadLettered',
  'private-reconciliation-failure-detail',
  'assert!(!debug.contains(PRIVATE_MARKER))',
  'assert!(!debug.contains(DEPENDENCY_CODE))',
  'assert!(!display.contains(PRIVATE_MARKER))',
  'assert!(!display.contains(DEPENDENCY_CODE))',
  'assert_eq!(calls.load(Ordering::SeqCst), 1)',
  'count(&evidence_db, "index_jobs")',
  'count(&evidence_db, "index_entities")',
  'count(&evidence_db, "index_inbox")',
  'DROP SCHEMA IF EXISTS',
]);
for (const forbidden of [
  'IndexReconciliationRunError::Source(',
  'tokio::time::sleep',
  'std::thread::sleep',
  'DELETE FROM index_jobs',
  'INSERT INTO index_jobs',
]) {
  if (target.includes(forbidden)) fail(`${targetPath} contains superseded or forbidden marker ${forbidden}`);
}

requireMarkers(docsPath, [
  'Status: `source_complete_operator_recovery_pending`.',
  'due pending retry work keep their existing reclaim path',
  'does not select `last_error_details`',
  '`IndexReconciliationRunStatus::FailedPermanent`',
  'does not call the source, insert another `index_jobs` row',
  'canonical M6 retry/global-scheduling and drift-diagnosis/targeted-repair roadmap items remain open',
  'maintainer-run',
]);
requireMarkers(docsIndexPath, [
  '[M6 Reconciliation Dead-letter Admission](./m6-reconciliation-dead-letter-admission.md)',
]);
requireMarkers(planPath, [
  '- [ ] Add bounded retry/backoff, dead-letter state, and global scheduling ownership.',
  '- [ ] Add drift diagnosis, targeted repair commands, and admitted repair evidence.',
]);
requireMarkers(aggregatePath, [
  "'verify-index-server-reconciliation-guard.mjs'",
  "'verify-index-reconciliation-runner-retry.mjs'",
  "'verify-index-reconciliation-dead-letter-admission.mjs'",
  "'verify-index-replay-dead-letter-admission.mjs'",
]);

console.log('[verify-index-reconciliation-dead-letter-admission] OK');

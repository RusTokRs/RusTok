#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const read = (relative) => fs.readFileSync(path.join(root, relative), 'utf8');
const fail = (message) => {
  console.error(`[verify-index-reconciliation-runner-retry] ${message}`);
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
const testsPath =
  'crates/rustok-index/src/infrastructure/postgres/source_reconciliation_runner_tests.rs';
const postgresTargetPath =
  'crates/rustok-index/tests/source_reconciliation_dead_letter_admission_postgres_test.rs';
const retryPath =
  'crates/rustok-index/src/infrastructure/postgres/source_reconciliation_retry.rs';
const docsPath =
  'crates/rustok-index/docs/m6-reconciliation-runner-retry-wiring.md';
const storeDocsPath =
  'crates/rustok-index/docs/m6-reconciliation-retry-transition-store.md';
const docsIndexPath = 'crates/rustok-index/docs/README.md';
const planPath = 'crates/rustok-index/docs/implementation-plan.md';
const aggregatePath = 'scripts/verify/verify-index-query-contract.mjs';

const runner = requireMarkers(runnerPath, [
  'IndexReconciliationRetryDisposition, IndexReconciliationRetryError,',
  'IndexReconciliationRetryFailure, IndexReconciliationRetryLease,',
  'PostgresIndexReconciliationRetryStore, PostgresMutationStore,',
  'RetryScheduled,',
  'FailedPermanent,',
  'FailedExhausted,',
  'retry_after: Option<Duration>,',
  'next_attempt: Option<u32>,',
  'pub fn retry_after(&self) -> Option<Duration>',
  'pub fn next_attempt(&self) -> Option<u32>',
  'async fn finish_page_error(',
  'fn retry_failure(',
  'RetryTransition(#[source] IndexReconciliationRetryError)',
]);

const production = runner.split('\n#[cfg(test)]')[0];
for (const forbidden of [
  'const RECONCILIATION_FAILURE_CONTRACT',
  'const RECONCILIATION_PAGE_FAILURE_CODE',
  'async fn finish_failure(',
  'fn finish_failure_sql(',
  'json!({',
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
    fail(`${runnerPath} production boundary contains forbidden marker ${forbidden}`);
  }
}

const callMarker = 'finish_page_error(&self.db, &lease, outcome, run_error).await';
if (production.split(callMarker).length - 1 !== 4) {
  fail(`${runnerPath} must route exactly four page-failure call sites through the retry boundary`);
}

const outcomeStart = production.indexOf('let mut outcome = IndexReconciliationRunOutcome {');
const loopStart = production.indexOf('for page_index in 0..request.max_pages', outcomeStart);
if (outcomeStart < 0 || loopStart <= outcomeStart) {
  fail(`${runnerPath} acquired outcome initialization is missing`);
}
const outcome = production.slice(outcomeStart, loopStart);
for (const marker of [
  'status: IndexReconciliationRunStatus::Yielded',
  'job_id: Some(lease.job_id)',
  'attempt_count: Some(lease.attempt_count)',
  'retry_after: None',
  'next_attempt: None',
]) {
  if (!outcome.includes(marker)) fail(`${runnerPath} acquired outcome is missing ${marker}`);
}

const finishStart = production.indexOf('async fn finish_page_error(');
const finishEnd = production.indexOf('\nasync fn yield_for_resume(', finishStart);
if (finishStart < 0 || finishEnd <= finishStart) {
  fail(`${runnerPath} retry completion boundary is malformed`);
}
const finish = production.slice(finishStart, finishEnd);
const classify = finish.indexOf('let failure = retry_failure(&error)?;');
const lease = finish.indexOf('let retry_lease = IndexReconciliationRetryLease::new(');
const store = finish.indexOf('let retry_store = PostgresIndexReconciliationRetryStore::new(db.clone());');
const record = finish.indexOf('retry_store.record_failure(&retry_lease, &failure).await');
if (classify < 0 || lease <= classify || store <= lease || record <= store) {
  fail(`${runnerPath} must classify, bind the exact lease, construct the store, then record failure`);
}
for (const marker of [
  'IndexReconciliationRetryDisposition::RetryScheduled {',
  'outcome.status = IndexReconciliationRunStatus::RetryScheduled;',
  'outcome.retry_after = Some(retry_after);',
  'outcome.next_attempt = Some(next_attempt);',
  'IndexReconciliationRetryDisposition::TerminalPermanent',
  'outcome.status = IndexReconciliationRunStatus::FailedPermanent;',
  'IndexReconciliationRetryDisposition::TerminalExhausted',
  'outcome.status = IndexReconciliationRunStatus::FailedExhausted;',
  'Err(IndexReconciliationRetryError::LeaseLost)',
  'if cancel_if_requested(db, lease).await?',
  'IndexReconciliationRunStatus::Cancelled',
  'IndexReconciliationRunError::LeaseLost {',
  'Err(error) => Err(IndexReconciliationRunError::RetryTransition(error))',
]) {
  if (!finish.includes(marker)) fail(`${runnerPath} retry completion is missing ${marker}`);
}
for (const forbidden of [
  'dependency_code',
  'last_error_details',
  'error.to_string()',
  'format!("{error',
]) {
  if (finish.includes(forbidden)) {
    fail(`${runnerPath} retry outcome boundary exposes forbidden marker ${forbidden}`);
  }
}

const classifyStart = production.indexOf('fn retry_failure(');
const classifyEnd = production.indexOf('\nfn empty_outcome(', classifyStart);
if (classifyStart < 0 || classifyEnd <= classifyStart) {
  fail(`${runnerPath} failure classifier is malformed`);
}
const classifier = production.slice(classifyStart, classifyEnd);
for (const marker of [
  'IndexReconciliationRunError::Source(IndexSourceError::SourceFailure',
  'IndexSourceFailureKind::Retryable',
  'IndexReconciliationRetryFailure::retryable(failure.code())',
  'IndexSourceFailureKind::Permanent',
  'IndexReconciliationRetryFailure::permanent(failure.code())',
  'IndexReconciliationRunError::MutationFailed',
  'IndexReplayFailureKind::Retryable',
  'IndexReplayFailureKind::Permanent',
  'IndexReconciliationRunError::Source(_) =>',
  'IndexReconciliationRetryFailure::permanent("source_contract_invalid")',
  'IndexReconciliationRetryFailure::permanent("reconciliation_contract_invalid")',
  'failure.map_err(IndexReconciliationRunError::RetryTransition)',
]) {
  if (!classifier.includes(marker)) fail(`${runnerPath} failure classifier is missing ${marker}`);
}

const retry = requireMarkers(retryPath, [
  'Self::new(5, Duration::from_secs(5), Duration::from_secs(300))',
  "state = 'pending'",
  "state = 'failed'",
  'available_at = {available_at}',
  'lease_owner = {prefix}3',
  'attempt_count = {prefix}4',
  'lease_expires_at > CURRENT_TIMESTAMP',
  'cancel_requested = FALSE',
]);
if (retry.includes('tokio::spawn') || retry.includes('tokio::time::sleep')) {
  fail(`${retryPath} must remain task-free and sleep-free`);
}

const tests = requireMarkers(testsPath, [
  'retryable_failure_schedules_due_attempts_and_terminally_exhausts',
  'permanent_failure_terminalizes_without_retry_metadata',
  'IndexReconciliationRunStatus::RetryScheduled',
  'IndexReconciliationRunStatus::FailedExhausted',
  'IndexReconciliationRunStatus::FailedPermanent',
  'Some(Duration::from_secs(seconds))',
  'assert_eq!(outcome.next_attempt(), Some(next_attempt));',
  'IndexReconciliationRunStatus::Busy',
  'fixture.make_retry_due().await;',
  'attempt_count: 5',
  'attempt_count: 1',
  'IndexReconciliationRunError::DeadLettered',
  "json_extract(last_error_details, '$.retryable')",
]);
for (const forbidden of ['tokio::time::sleep', 'std::thread::sleep']) {
  if (tests.includes(forbidden)) fail(`${testsPath} contains forbidden timing shortcut ${forbidden}`);
}

const postgresTarget = requireMarkers(postgresTargetPath, [
  'failed_reconciliation_scope_blocks_new_jobs_without_exposing_details',
  'IndexReconciliationRunStatus::FailedPermanent',
  'first_outcome.retry_after(), None',
  'first_outcome.next_attempt(), None',
  'IndexReconciliationRunError::DeadLettered',
  'private-reconciliation-failure-detail',
]);
if (postgresTarget.includes('IndexReconciliationRunError::Source(')) {
  fail(`${postgresTargetPath} still expects the superseded first-run error contract`);
}

requireMarkers(docsPath, [
  'Status: `runner_complete_host_scheduler_pending`.',
  '`RetryScheduled`',
  '`FailedPermanent`',
  '`FailedExhausted`',
  'attempt 1 failure -> pending for 5 seconds',
  'attempt 5 retryable failure -> failed exhausted',
  'An invocation before `available_at` receives the existing `Busy` outcome',
  'The canonical retry/backoff/dead-letter/global-scheduling roadmap item remains open',
  'maintainer-run',
]);
requireMarkers(storeDocsPath, [
  'Status: `runner_complete_scheduler_wiring_pending`.',
  '`PostgresIndexReconciliationRunner` now classifies page failures',
  'global scheduling ownership is not part of this slice',
]);
requireMarkers(docsIndexPath, [
  '[M6 Reconciliation Runner Retry Wiring](./m6-reconciliation-runner-retry-wiring.md)',
]);
requireMarkers(planPath, [
  '- [ ] Add bounded retry/backoff, dead-letter state, and global scheduling ownership.',
  '- [ ] Add drift diagnosis, targeted repair commands, and admitted repair evidence.',
]);
requireMarkers(aggregatePath, [
  "'verify-index-reconciliation-retry-store.mjs'",
  "'verify-index-reconciliation-runner-retry.mjs'",
  "'verify-index-reconciliation-dead-letter-admission.mjs'",
]);

console.log('[verify-index-reconciliation-runner-retry] OK');

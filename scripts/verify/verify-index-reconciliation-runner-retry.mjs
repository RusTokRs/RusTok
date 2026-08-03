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
const schedulerPath =
  'crates/rustok-index/src/infrastructure/postgres/source_reconciliation_scheduler.rs';
const docsPath =
  'crates/rustok-index/docs/m6-reconciliation-runner-retry-wiring.md';
const schedulerDocsPath =
  'crates/rustok-index/docs/m6-reconciliation-host-scheduler.md';
const testsPath =
  'crates/rustok-index/src/infrastructure/postgres/source_reconciliation_runner_tests.rs';
const planPath = 'crates/rustok-index/docs/implementation-plan.md';
const aggregatePath = 'scripts/verify/verify-index-query-contract.mjs';

const runner = requireMarkers(runnerPath, [
  'IndexReconciliationRetryDisposition, IndexReconciliationRetryError,',
  'PostgresIndexReconciliationRetryStore, PostgresMutationStore,',
  'RetryScheduled,', 'FailedPermanent,', 'FailedExhausted,',
  'retry_after: Option<Duration>,', 'next_attempt: Option<u32>,',
  'async fn finish_page_error(', 'fn retry_failure(',
  'RetryTransition(#[source] IndexReconciliationRetryError)',
]);
const production = runner.split('\n#[cfg(test)]')[0];
for (const forbidden of [
  'async fn finish_failure(', 'fn finish_failure_sql(', 'tokio::spawn',
  'tokio::time::sleep', 'std::thread::sleep', 'loop {', 'Router::new',
]) {
  if (production.includes(forbidden)) fail(`${runnerPath} contains ${forbidden}`);
}
const callMarker = 'finish_page_error(&self.db, &lease, outcome, run_error).await';
if (production.split(callMarker).length - 1 !== 4) {
  fail(`${runnerPath} must retain exactly four page-failure call sites`);
}
for (const marker of [
  'IndexReconciliationRetryDisposition::RetryScheduled {',
  'outcome.status = IndexReconciliationRunStatus::RetryScheduled;',
  'IndexReconciliationRetryDisposition::TerminalPermanent',
  'outcome.status = IndexReconciliationRunStatus::FailedPermanent;',
  'IndexReconciliationRetryDisposition::TerminalExhausted',
  'outcome.status = IndexReconciliationRunStatus::FailedExhausted;',
  'Err(IndexReconciliationRetryError::LeaseLost)',
  'if cancel_if_requested(db, lease).await?',
  'IndexReconciliationRetryFailure::permanent("source_contract_invalid")',
  'IndexReconciliationRetryFailure::permanent("reconciliation_contract_invalid")',
]) {
  if (!production.includes(marker)) fail(`${runnerPath} is missing ${marker}`);
}

requireMarkers(schedulerPath, [
  'IndexReconciliationRunRequest::new(',
  '.runner',
  '.run(request)',
  'IndexReconciliationRunStatus::Busy',
  'IndexReconciliationRunStatus::RetryScheduled',
  'IndexReconciliationRunStatus::FailedPermanent',
  'IndexReconciliationRunStatus::FailedExhausted',
]);
requireMarkers(testsPath, [
  'retryable_failure_schedules_due_attempts_and_terminally_exhausts',
  'permanent_failure_terminalizes_without_retry_metadata',
  'IndexReconciliationRunStatus::RetryScheduled',
  'IndexReconciliationRunStatus::FailedExhausted',
  'IndexReconciliationRunStatus::FailedPermanent',
]);
requireMarkers(docsPath, [
  'Status: `host_scheduler_source_complete_owner_execution_pending`.',
  '`RetryScheduled`', '`FailedPermanent`', '`FailedExhausted`',
  'attempt 1 failure -> pending for 5 seconds',
  'The generic `ModuleWorkScheduler` owns polling cadence and graceful StopHandle shutdown',
  'maintainer-run',
]);
requireMarkers(schedulerDocsPath, [
  'The adapter never claims `index_jobs` directly',
  'The runner continues to own',
]);
requireMarkers(planPath, [
  '- [ ] Add bounded retry/backoff, dead-letter state, and global scheduling ownership.',
  '- [ ] Add drift diagnosis, targeted repair commands, and admitted repair evidence.',
]);
requireMarkers(aggregatePath, [
  "'verify-index-reconciliation-runner-retry.mjs'",
  "'verify-index-reconciliation-host-scheduler.mjs'",
]);

console.log('[verify-index-reconciliation-runner-retry] OK');

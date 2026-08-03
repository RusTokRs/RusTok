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
const runnerPath =
  'crates/rustok-index/src/infrastructure/postgres/source_reconciliation_runner.rs';
const schedulerPath =
  'crates/rustok-index/src/infrastructure/postgres/source_reconciliation_scheduler.rs';
const docsPath =
  'crates/rustok-index/docs/m6-reconciliation-retry-transition-store.md';
const schedulerDocsPath =
  'crates/rustok-index/docs/m6-reconciliation-host-scheduler.md';
const planPath = 'crates/rustok-index/docs/implementation-plan.md';
const aggregatePath = 'scripts/verify/verify-index-query-contract.mjs';

const retry = requireMarkers(retryPath, [
  'const MAX_RECONCILIATION_ATTEMPTS: u32 = 100;',
  'const MAX_BACKOFF_SECONDS: u64 = 86_400;',
  'const RECONCILIATION_FAILURE_CONTRACT: &str = "index_reconciliation_run_failure_v1";',
  'const RECONCILIATION_PAGE_FAILURE_CODE: &str = "index.reconciliation_page_failed";',
  'pub struct IndexReconciliationRetryPolicy',
  'pub struct PostgresIndexReconciliationRetryStore',
  'IndexReconciliationRetryDisposition::RetryScheduled',
  'IndexReconciliationRetryDisposition::TerminalPermanent',
  'IndexReconciliationRetryDisposition::TerminalExhausted',
  'Self::new(5, Duration::from_secs(5), Duration::from_secs(300))',
]);
const production = retry.split('\n#[cfg(test)]')[0];
for (const forbidden of [
  'INSERT INTO index_jobs', 'DELETE FROM index_jobs', 'tokio::spawn',
  'tokio::time::sleep', 'std::thread::sleep', 'loop {', 'Router::new',
]) {
  if (production.includes(forbidden)) fail(`${retryPath} contains ${forbidden}`);
}
for (const marker of [
  "state = 'pending'", 'available_at = {available_at}',
  "state = 'failed'", 'completed_at = CURRENT_TIMESTAMP',
  'lease_owner = {prefix}3', 'attempt_count = {prefix}4',
  'lease_expires_at > CURRENT_TIMESTAMP', 'cancel_requested = FALSE',
  '"contract": RECONCILIATION_FAILURE_CONTRACT',
  '"dependency_code": failure.code()',
]) {
  if (!production.includes(marker)) fail(`${retryPath} is missing ${marker}`);
}

requireMarkers(runnerPath, [
  'PostgresIndexReconciliationRetryStore, PostgresMutationStore,',
  'IndexReconciliationRetryLease::new(',
  'retry_store.record_failure(&retry_lease, &failure).await',
  'IndexReconciliationRunStatus::RetryScheduled',
  'IndexReconciliationRunStatus::FailedPermanent',
  'IndexReconciliationRunStatus::FailedExhausted',
]);
requireMarkers(schedulerPath, [
  'PostgresIndexReconciliationRunner',
  '.runner',
  '.run(request)',
  'impl ModuleWorkRegistration for IndexReconciliationWorkRegistration',
]);
requireMarkers(docsPath, [
  'Status: `host_scheduler_source_complete_owner_execution_pending`.',
  'maximum attempts: `5`',
  'delays after attempts 1-4: `5`, `10`, `20`, and `40` seconds',
  'Fleet duplicates remain safe',
  'maintainer-run',
]);
requireMarkers(schedulerDocsPath, [
  'Status: `source_complete_owner_execution_pending`.',
  'The generic host scheduler remains the only polling and lifecycle owner',
]);
requireMarkers(planPath, [
  '- [ ] Add bounded retry/backoff, dead-letter state, and global scheduling ownership.',
  '- [ ] Add drift diagnosis, targeted repair commands, and admitted repair evidence.',
]);
requireMarkers(aggregatePath, [
  "'verify-index-reconciliation-retry-store.mjs'",
  "'verify-index-reconciliation-host-scheduler.mjs'",
]);

console.log('[verify-index-reconciliation-retry-store] OK');

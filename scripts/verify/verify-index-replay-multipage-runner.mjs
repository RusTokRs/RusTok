#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const read = (relative) => fs.readFileSync(path.join(root, relative), 'utf8');
const fail = (message) => {
  console.error(`[verify-index-replay-multipage-runner] ${message}`);
  process.exit(1);
};
const requireMarkers = (relative, markers) => {
  const source = read(relative);
  for (const marker of markers) {
    if (!source.includes(marker)) fail(`${relative} is missing ${marker}`);
  }
  return source;
};

const runnerPath = 'crates/rustok-index/src/infrastructure/postgres/source_replay_runner.rs';
const runner = requireMarkers(runnerPath, [
  'const MAX_PAGES_PER_RUN: usize = 1_024;',
  'pub struct IndexReplayRunRequest',
  'pub enum IndexReplayRunStatus',
  'pub struct IndexReplayRunOutcome',
  'pub struct PostgresIndexReplayRunner',
  '.source_for_schema(request.page_request().schema())',
  'IndexReplayJobLeaseRequest::new(',
  'PostgresIndexReplayCheckpointStore::new(self.db.clone(), lease.clone())',
  'for page_index in 0..request.max_pages()',
  'page_index % request.heartbeat_every_pages() == 0',
  'worker.run_next_page(request.page_request().clone())',
  'job_store.succeed(&lease).await',
  'yield_for_resume(&self.db, &lease).await?;',
  "state = 'pending'",
  "kind = 'rebuild'",
  'lease_expires_at > CURRENT_TIMESTAMP',
  'index_replay_run_failure_v1',
  'checkpoint_lease_lost',
  'IndexReplayRunError::LeaseLost',
]);

for (const forbidden of [
  'tokio::spawn',
  'loop {',
  'tokio::time::sleep',
  'cancel_requested',
  'DELETE FROM index_jobs',
  'rustok_product',
  'rustok_content',
  'rustok_flex',
]) {
  if (runner.includes(forbidden)) fail(`${runnerPath} contains forbidden marker ${forbidden}`);
}

const runLoop = runner.indexOf('for page_index in 0..request.max_pages()');
const pageCall = runner.indexOf('worker.run_next_page(request.page_request().clone())', runLoop);
const successCall = runner.indexOf('job_store.succeed(&lease).await', pageCall);
const yieldCall = runner.indexOf('yield_for_resume(&self.db, &lease).await?;', successCall);
if (runLoop < 0 || pageCall <= runLoop || successCall <= pageCall || yieldCall <= successCall) {
  fail('runner must process bounded pages, complete on null cursor, then yield unfinished work');
}

requireMarkers('crates/rustok-index/src/infrastructure/postgres/source_replay_runner_tests.rs', [
  'bounded_run_yields_pending_and_resumes_with_a_new_attempt',
  'lease_loss_during_a_page_does_not_publish_failure_or_advance_cursor',
  'run_request_bounds_pages_and_heartbeat_cadence',
  'IndexReplayRunStatus::Yielded',
  'IndexReplayRunStatus::Complete',
  'second.attempt_count(), Some(2)',
  "state = 'pending'",
  'last_error_code IS NULL',
]);

requireMarkers('crates/rustok-index/src/infrastructure/postgres/mod.rs', [
  'mod source_replay_runner;',
  'mod source_replay_runner_tests;',
  'PostgresIndexReplayRunner',
  'IndexReplayRunRequest',
  'IndexReplayRunOutcome',
]);

requireMarkers('crates/rustok-index/src/lib.rs', [
  'bounded multi-page',
  'PostgresIndexReplayRunner',
  'IndexReplayRunRequest',
  'IndexReplayRunStatus',
]);

requireMarkers('crates/rustok-index/docs/m6-bounded-multipage-runner.md', [
  'Status: `source_complete_owner_execution_pending`',
  '1 through 1024 pages per invocation',
  'The source name is never caller supplied.',
  'returns the same job to',
  '`pending`',
  '`index_replay_run_failure_v1`',
  'cancellation request observation',
  'maintainer-run',
]);

requireMarkers('crates/rustok-index/docs/implementation-plan.md', [
  '- M6 bounded multi-page replay runner: `source_complete_owner_execution_pending`',
  '- [x] Add bounded multi-page execution with heartbeat cadence and immediate pending resume.',
  'Cancellation, automatic retry/backoff, dead-letter scheduling, host scheduling, and',
]);

requireMarkers('scripts/verify/verify-index-query-contract.mjs', [
  "'verify-index-replay-job-leases.mjs'",
  "'verify-index-replay-multipage-runner.mjs'",
  "'verify-index-source-replay-contract.mjs'",
]);

console.log('[verify-index-replay-multipage-runner] OK');

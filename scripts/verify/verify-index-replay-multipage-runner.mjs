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
  'Cancelled,',
  'pub enum IndexReplayCancelOutcome',
  'pub enum IndexReplayTerminalState',
  'pub struct PostgresIndexReplayRunner',
  'pub async fn request_cancel(',
  '.source_for_schema(request.page_request().schema())',
  'IndexReplayJobLeaseRequest::new(',
  'PostgresIndexReplayCheckpointStore::new(self.db.clone(), lease.clone())',
  'for page_index in 0..request.max_pages()',
  'page_index % request.heartbeat_every_pages() == 0',
  'worker.run_next_page(request.page_request().clone())',
  'cancel_if_requested(&self.db, &lease).await?',
  'finish_success(&self.db, &lease).await?',
  'finish_failure(&self.db, &lease, details).await?',
  'yield_for_resume(&self.db, &lease).await?',
  "state = 'pending'",
  "state = 'cancelled'",
  'cancel_requested = TRUE',
  'cancel_requested = FALSE',
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
  'DELETE FROM index_jobs',
  'rustok_product',
  'rustok_content',
  'rustok_flex',
  'run_interruptible<Check>',
]) {
  if (runner.includes(forbidden)) fail(`${runnerPath} contains forbidden marker ${forbidden}`);
}

const runLoop = runner.indexOf('for page_index in 0..request.max_pages()');
const prePageCancel = runner.indexOf('cancel_if_requested(&self.db, &lease).await?', runLoop);
const pageCall = runner.indexOf('worker.run_next_page(request.page_request().clone())', runLoop);
const postPageCancel = runner.indexOf('cancel_if_requested(&self.db, &lease).await?', pageCall);
const successCall = runner.indexOf('finish_success(&self.db, &lease).await?', postPageCancel);
const yieldCall = runner.indexOf('yield_for_resume(&self.db, &lease).await?', successCall);
if (
  runLoop < 0
  || prePageCancel <= runLoop
  || pageCall <= prePageCancel
  || postPageCancel <= pageCall
  || successCall <= postPageCancel
  || yieldCall <= successCall
) {
  fail('ordinary runner must preserve cancellation before/after pages, complete on null cursor, then yield');
}

for (const terminalSql of ['finish_success_sql', 'finish_failure_sql', 'yield_job_sql']) {
  const start = runner.indexOf(`fn ${terminalSql}(`);
  const end = runner.indexOf('\n}', start);
  if (start < 0 || !runner.slice(start, end).includes('cancel_requested = FALSE')) {
    fail(`${terminalSql} must make cancellation win over terminal or pending publication`);
  }
}

requireMarkers('crates/rustok-index/src/infrastructure/postgres/source_replay_runner_tests.rs', [
  'bounded_run_yields_pending_and_resumes_with_a_new_attempt',
  'pending_cancel_request_terminalizes_without_a_worker',
  'running_cancel_request_is_observed_after_the_current_page',
  'requested_running_cancel_survives_reclaim_and_fences_the_old_attempt',
  'lease_loss_during_a_page_does_not_publish_failure_or_advance_cursor',
  'run_request_bounds_pages_and_heartbeat_cadence',
  'IndexReplayCancelOutcome::Cancelled',
  'IndexReplayCancelOutcome::Requested',
  'IndexReplayRunStatus::Cancelled',
  'second.attempt_count(), Some(2)',
  'cancel_requested = TRUE',
  'last_error_code IS NULL',
]);

requireMarkers('crates/rustok-index/src/infrastructure/postgres/mod.rs', [
  'mod source_replay_runner {',
  'include!("source_replay_runner.rs");',
  'mod graceful_shutdown;',
  'mod source_replay_runner_tests;',
  'mod source_replay_graceful_shutdown_tests;',
  'PostgresIndexReplayRunner',
  'IndexReplayRunRequest',
  'IndexReplayRunOutcome',
  'IndexReplayCancelOutcome',
  'IndexReplayTerminalState',
]);

requireMarkers('crates/rustok-index/src/lib.rs', [
  'heartbeat/yield/cancellation semantics',
  'PostgresIndexReplayRunner',
  'IndexReplayRunRequest',
  'IndexReplayRunStatus',
  'IndexReplayCancelOutcome',
]);

requireMarkers('crates/rustok-index/docs/m6-bounded-multipage-runner.md', [
  'Status: `source_complete_owner_execution_pending`',
  '1 through 1024 pages per invocation',
  'The source name is never caller supplied.',
  '`PostgresIndexReplayRunner::request_cancel`',
  '`PostgresIndexReplayRunner::run_interruptible`',
  'Host interruption does not set `cancel_requested`',
  'a `pending` job becomes terminal `cancelled` immediately',
  'cancel_requested = FALSE',
  'A running cancellation request survives lease expiry and reclaim.',
  'server-owned `StopHandle`',
  'maintainer-run',
]);

requireMarkers('crates/rustok-index/docs/implementation-plan.md', [
  '- M6 bounded multi-page replay and cancellation: `source_complete_owner_execution_pending`',
  '- [x] Add bounded multi-page execution with heartbeat cadence and immediate pending resume.',
  '- [x] Add durable cancellation requests and fenced between-page terminal cancellation.',
]);

requireMarkers('scripts/verify/verify-index-query-contract.mjs', [
  "'verify-index-replay-job-leases.mjs'",
  "'verify-index-replay-multipage-runner.mjs'",
  "'verify-index-replay-graceful-shutdown.mjs'",
  "'verify-index-source-replay-contract.mjs'",
]);

console.log('[verify-index-replay-multipage-runner] ordinary replay/cancel semantics remain stable while host-probed interruption is isolated in a separate extension');

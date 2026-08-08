#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const read = (relative) => fs.readFileSync(path.join(root, relative), 'utf8');
const fail = (message) => {
  console.error(`[verify-index-replay-page-lease-heartbeat] ${message}`);
  process.exit(1);
};
const requireMarkers = (relative, markers) => {
  const source = read(relative);
  for (const marker of markers) {
    if (!source.includes(marker)) fail(`${relative} is missing ${marker}`);
  }
  return source;
};

const sourceTimeoutPath = 'crates/rustok-index/src/application/source_timeout.rs';
requireMarkers(sourceTimeoutPath, [
  'DEFAULT_INDEX_SOURCE_CALL_TIMEOUT: Duration = Duration::from_secs(30)',
  'INDEX_SOURCE_SCAN_TIMEOUT_CODE: &str = "index_source_scan_timeout"',
  'TimedIndexSource::new(source, DEFAULT_INDEX_SOURCE_CALL_TIMEOUT)',
]);

const storageTimeoutPath = 'crates/rustok-index/src/infrastructure/postgres/source_replay_timeout.rs';
requireMarkers(storageTimeoutPath, [
  'DEFAULT_INDEX_REPLAY_STORAGE_FUTURE_TIMEOUT: Duration = Duration::from_secs(30)',
  'INDEX_REPLAY_CHECKPOINT_READ_TIMEOUT_CODE: &str = "index_replay_checkpoint_read_timeout"',
  'INDEX_REPLAY_MUTATION_TIMEOUT_CODE: &str = "index_replay_mutation_timeout"',
  '"index_replay_checkpoint_commit_timeout"',
  'bounded_replay_checkpoint_read',
  'bounded_replay_mutation',
  'bounded_replay_checkpoint_commit',
]);

const runnerPath = 'crates/rustok-index/src/infrastructure/postgres/source_replay_runner.rs';
const runner = requireMarkers(runnerPath, [
  'const MIN_REPLAY_RUN_LEASE_DURATION: Duration = Duration::from_secs(60);',
  'const PAGE_LEASE_HEARTBEAT_DIVISOR: u32 = 3;',
  'if lease_duration < MIN_REPLAY_RUN_LEASE_DURATION {',
  'IndexReplayRunError::LeaseDurationTooShort',
  'fn page_lease_heartbeat_interval(lease_duration: Duration) -> Duration',
  'lease_duration / PAGE_LEASE_HEARTBEAT_DIVISOR',
  'async fn await_page_with_lease_heartbeats<T, F>(',
  'tokio::select!',
  'sleep_until(next_heartbeat)',
  'heartbeat(job_store, lease, lease_duration)',
  'aggregate.heartbeat_count += in_page_heartbeat_count;',
  'page_index % request.heartbeat_every_pages() == 0',
  'Duration::from_secs(59)',
  'Duration::from_secs(60)',
  'Duration::from_secs(20)',
]);
for (const forbidden of [
  'index_replay_page_timeout',
  'IndexReplayRunStatus::Retrying',
  'automatic_retry',
  'auto_requeue',
]) {
  if (runner.includes(forbidden)) fail(`${runnerPath} absorbed forbidden page/retry semantics: ${forbidden}`);
}

const ordinaryHelper = runner.indexOf('let (page_result, in_page_heartbeat_count) = await_page_with_lease_heartbeats(');
const ordinaryPage = runner.indexOf('worker.run_next_page(request.page_request().clone())', ordinaryHelper);
const ordinaryMatch = runner.indexOf('let page = match page_result {', ordinaryPage);
if (ordinaryHelper < 0 || ordinaryPage <= ordinaryHelper || ordinaryMatch <= ordinaryPage) {
  fail('ordinary replay page must be nested inside the common lease-heartbeat await before result handling');
}

const gracefulPath = 'crates/rustok-index/src/infrastructure/postgres/source_replay_runner/graceful_shutdown.rs';
const graceful = requireMarkers(gracefulPath, [
  'let page_future = worker.run_next_page_interruptible(',
  'await_page_with_lease_heartbeats(',
  'aggregate.heartbeat_count += in_page_heartbeat_count;',
  'Err(crate::IndexReplayError::Interrupted) => {',
  'yield_after_host_interruption(&self.db, &lease, aggregate).await',
]);
if (graceful.includes('index_replay_page_timeout')) {
  fail(`${gracefulPath} must not add a generic page timeout`);
}

requireMarkers('apps/server/src/graphql/index_replay.rs', [
  'const GRAPHQL_REPLAY_LEASE_SECONDS: u64 = 60;',
  'Duration::from_secs(GRAPHQL_REPLAY_LEASE_SECONDS)',
]);

requireMarkers('crates/rustok-index/docs/m6-replay-page-lease-heartbeat.md', [
  'Status: `source_complete_execution_pending`.',
  '`index_replay_checkpoint_read_timeout`',
  '`IndexReplayRunRequest` now rejects lease durations shorter than `60s`',
  'one third of the configured lease duration',
  '`60s` lease this produces a `20s` in-page heartbeat interval',
  'There is no new page terminal state and no generic `index_replay_page_timeout` code.',
  'persisted cancellation',
  'graceful interruption',
  'The retained Rust tests and Node verifiers were not executed',
]);

requireMarkers('crates/rustok-index/docs/m6-replay-pending-future-timeouts.md', [
  '`index_replay_checkpoint_read_timeout`',
  '`IndexReplayRunRequest` requires at least a 60-second lease',
  '`m6-replay-page-lease-heartbeat.md`',
  'There is deliberately no generic `index_replay_page_timeout` code.',
]);

requireMarkers('crates/rustok-index/docs/implementation-plan-current-2026-08-08.md', [
  'Define/retain whole-page duration versus lease/heartbeat policy beyond per-dependency bounds',
  'Complete remaining multi-host/restart evidence beyond existing convergence/replay packets',
  'Add partition replay scope only after a real partition-capable source contract exists',
]);

console.log('[verify-index-replay-page-lease-heartbeat] replay pages retain exact dependency timeouts while ordinary/graceful runners maintain a minimum-60s fenced lease every one-third lease interval');

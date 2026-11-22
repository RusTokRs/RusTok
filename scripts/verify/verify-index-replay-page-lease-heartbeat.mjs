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

const sourceTimeoutPath = 'crates/modules/rustok-index/src/application/source_timeout.rs';
requireMarkers(sourceTimeoutPath, [
  'DEFAULT_INDEX_SOURCE_CALL_TIMEOUT: Duration = Duration::from_secs(30)',
  'INDEX_SOURCE_SCAN_TIMEOUT_CODE: &str = "index_source_scan_timeout"',
  'TimedIndexSource::new(source, DEFAULT_INDEX_SOURCE_CALL_TIMEOUT)',
]);

const storageTimeoutPath = 'crates/modules/rustok-index/src/infrastructure/postgres/source_replay_timeout.rs';
requireMarkers(storageTimeoutPath, [
  'DEFAULT_INDEX_REPLAY_STORAGE_FUTURE_TIMEOUT: Duration = Duration::from_secs(30)',
  'INDEX_REPLAY_CHECKPOINT_READ_TIMEOUT_CODE: &str = "index_replay_checkpoint_read_timeout"',
  'INDEX_REPLAY_MUTATION_TIMEOUT_CODE: &str = "index_replay_mutation_timeout"',
  '"index_replay_checkpoint_commit_timeout"',
  'bounded_replay_checkpoint_read',
  'bounded_replay_mutation',
  'bounded_replay_checkpoint_commit',
]);

const pagePath = 'crates/modules/rustok-index/src/infrastructure/postgres/source_job_page.rs';
const driver = requireMarkers(pagePath, [
  'const PAGE_DEADLINE: Duration = Duration::from_secs(90);',
  'lease_duration / 3',
  'pub(super) async fn drive_page<',
  'sleep_until(deadline)',
  'sleep_until(next_heartbeat)',
  'let heartbeat = refresh();',
  'result = &mut page',
  'result = &mut heartbeat',
]);
if (driver.indexOf('result = &mut page', driver.indexOf('let heartbeat = refresh();')) < 0) {
  fail('a blocked heartbeat must not stop polling the page that can release its job row');
}
const runner = requireMarkers('crates/modules/rustok-index/src/infrastructure/postgres/source_replay_runner.rs', [
  'const MIN_REPLAY_RUN_LEASE_DURATION: Duration = Duration::from_secs(60);',
  'IndexReplayRunError::LeaseDurationTooShort',
  'self.run_interruptible(request, || false).await',
  'let page = drive_page(',
  'worker.run_next_page_interruptible(',
  'aggregate.heartbeat_count += in_page_heartbeat_count;',
  'replay_page_deadline',
  'page_index % request.heartbeat_every_pages() == 0',
  'Duration::from_secs(20)',
]);
if (runner.indexOf('worker.run_next_page_interruptible(') < runner.indexOf('let page = drive_page(')) {
  fail('ordinary and interrupted replay must use the one shared page driver');
}
requireMarkers('crates/modules/rustok-index/src/infrastructure/postgres/source_reconciliation_runner.rs', ['drive_page(request.lease_duration(), page_work']);

requireMarkers('apps/server/src/graphql/index_replay.rs', [
  'const GRAPHQL_REPLAY_LEASE_SECONDS: u64 = 60;',
  'Duration::from_secs(GRAPHQL_REPLAY_LEASE_SECONDS)',
]);

requireMarkers('crates/modules/rustok-index/docs/m6-replay-page-lease-heartbeat.md', [
  'Status: `source_complete_execution_pending`.',
  '`index_replay_checkpoint_read_timeout`',
  '`IndexReplayRunRequest` now rejects lease durations shorter than `60s`',
  'one third of the configured lease duration',
  '`60s` lease this produces a `20s` in-page heartbeat interval',
  'The 90-second observation deadline is separate from the shorter dependency timeouts.',
  'persisted cancellation',
  'graceful interruption',
  'The retained Rust tests and Node verifiers were not executed',
]);

requireMarkers('crates/modules/rustok-index/docs/m6-replay-pending-future-timeouts.md', [
  '`index_replay_checkpoint_read_timeout`',
  '`IndexReplayRunRequest` requires at least a 60-second lease',
  '`m6-replay-page-lease-heartbeat.md`',
  'replay_page_deadline',
]);

requireMarkers('crates/modules/rustok-index/docs/implementation-plan.md', [
  'Define/retain whole-page duration versus lease/heartbeat policy beyond per-dependency bounds',
  'Retain deterministic two-host lease-expiry/reclaim/stale-owner fencing evidence through distinct replay runners.',
  'Execute/admit retained multi-host reclaim evidence.',
  'Add partition replay scope only after a real partition-capable source contract exists',
]);

console.log('[verify-index-replay-page-lease-heartbeat] replay pages retain exact dependency timeouts while ordinary/graceful runners maintain a minimum-60s fenced lease every one-third lease interval');

#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const read = (relative) => fs.readFileSync(path.join(root, relative), 'utf8');
const fail = (message) => {
  console.error(`[verify-index-replay-page-interruption] ${message}`);
  process.exit(1);
};
const requireMarkers = (relative, markers) => {
  const source = read(relative);
  for (const marker of markers) {
    if (!source.includes(marker)) fail(`${relative} is missing ${marker}`);
  }
  return source;
};

const replayPath = 'crates/rustok-index/src/application/source_replay.rs';
const replay = requireMarkers(replayPath, [
  'future::Future',
  'pub async fn run_next_page_interruptible<Check, CheckFuture>',
  'Check: FnMut() -> CheckFuture',
  'CheckFuture: Future<Output = Result<bool, IndexReplayFailure>>',
  'Ok::<bool, IndexReplayFailure>(false)',
  'Ok(true) => Err(IndexReplayError::Interrupted)',
  'Err(failure) => Err(IndexReplayError::InterruptionCheckFailed(failure))',
  'Index replay page was cooperatively interrupted',
  'Index replay interruption check failed',
]);
const interruptionChecks = replay.match(
  /check_replay_interruption\(&mut should_interrupt\)\.await\?;/g,
)?.length ?? 0;
if (interruptionChecks !== 3) {
  fail(`${replayPath} must retain exactly three cooperative interruption boundaries`);
}
for (const forbidden of [
  'DatabaseConnection',
  'index_jobs',
  'cancel_requested',
  'tokio::spawn',
  'tokio::time::sleep',
  'loop {',
  'last_error_details',
  'backtrace',
  'stack_trace',
]) {
  if (replay.includes(forbidden)) {
    fail(`${replayPath} contains forbidden host/storage/scheduler marker ${forbidden}`);
  }
}

requireMarkers('crates/rustok-index/src/application/source_replay_tests.rs', [
  'interruption_before_source_scan_skips_source_and_checkpoint',
  'interruption_before_checkpoint_replays_applied_mutation_without_advancing_cursor',
  'interruption_probe_failure_stays_bounded_and_skips_source',
  'Err(IndexReplayError::Interrupted)',
  'Err::<bool, IndexReplayFailure>',
  'assert!(checkpoint.lock().unwrap().is_none());',
  'assert_eq!(outcome.duplicate_count(), 1);',
]);

requireMarkers('crates/rustok-index/docs/m6-cooperative-page-interruption.md', [
  'Status: `source_complete_runner_probe_pending`',
  '`run_next_page_interruptible`',
  '`IndexReplayError::Interrupted`',
  '`IndexReplayError::InterruptionCheckFailed`',
  'before every mutation',
  'never commits the page checkpoint',
  '`(tenant_id, job_id, worker_id, attempt_count)`',
  'cannot preempt one indefinitely pending source or mutation future',
  'combined implementation-plan item for in-page interruption/timeouts therefore\nremains open',
  'maintainer-run',
]);

console.log('[verify-index-replay-page-interruption] OK');

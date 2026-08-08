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

const checkStatement = 'check_replay_interruption(&mut should_interrupt).await?;';
const interruptionChecks = replay.match(
  /check_replay_interruption\(&mut should_interrupt\)\.await\?;/g,
)?.length ?? 0;
if (interruptionChecks !== 3) {
  fail(`${replayPath} must retain exactly three cooperative interruption call sites`);
}

const runStart = replay.indexOf('pub async fn run_next_page_interruptible<Check, CheckFuture>');
const firstCheck = replay.indexOf(checkStatement, runStart);
const scanStart = replay.indexOf('let scan_request = IndexSourceScanRequest::new(', runStart);
const mutationState = replay.indexOf('let mut applied_count = 0;', scanStart);
const mutationLoop = replay.indexOf(
  'for (position, mutation) in page.mutations().iter().enumerate() {',
  mutationState,
);
const mutationCheck = replay.indexOf(checkStatement, mutationLoop);
const mutationApply = replay.indexOf('.apply_replay_mutation(', mutationCheck);
const checkpointBuild = replay.indexOf('let checkpoint = IndexReplayCheckpoint::new(', mutationApply);
const checkpointCheck = replay.indexOf(checkStatement, checkpointBuild);
const checkpointCommit = replay.indexOf('.commit_replay_checkpoint(&checkpoint)', checkpointCheck);
if (
  runStart < 0
  || firstCheck < runStart
  || scanStart <= firstCheck
  || mutationState <= scanStart
  || mutationLoop <= mutationState
  || mutationCheck <= mutationLoop
  || mutationApply <= mutationCheck
  || checkpointBuild <= mutationApply
  || checkpointCheck <= checkpointBuild
  || checkpointCommit <= checkpointCheck
) {
  fail(`${replayPath} must check before scan, before each mutation, and before checkpoint commit`);
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

requireMarkers('crates/rustok-index/src/application/source_timeout.rs', [
  'const DEFAULT_INDEX_SOURCE_CALL_TIMEOUT: Duration = Duration::from_secs(30);',
  'const INDEX_SOURCE_SCAN_TIMEOUT_CODE: &str = "index_source_scan_timeout";',
  'TimedIndexSource::new(source, DEFAULT_INDEX_SOURCE_CALL_TIMEOUT)',
]);
requireMarkers('crates/rustok-index/src/replay_dry_run.rs', [
  'pub struct SharedIndexReplayDryRunRuntime',
  '.scan(scan_request)',
]);

const runnerPath = 'crates/rustok-index/src/infrastructure/postgres/source_replay_runner.rs';
const runner = read(runnerPath);
if (runner.includes('run_next_page_interruptible')) {
  fail(`${runnerPath} ordinary replay runner file must stay separate from the interruption extension`);
}

const runnerExtensionPath =
  'crates/rustok-index/src/infrastructure/postgres/source_replay_runner/graceful_shutdown.rs';
const runnerExtension = requireMarkers(runnerExtensionPath, [
  'pub async fn run_interruptible<Check>(',
  '.run_next_page_interruptible(request.page_request().clone(), || {',
  'Err(crate::IndexReplayError::Interrupted) => {',
  'yield_after_host_interruption(&self.db, &lease, aggregate).await',
  'match yield_for_resume(db, lease).await?',
]);
for (const forbidden of ['request_cancel(', 'cancel_requested = TRUE', 'finish_failure(db, lease']) {
  if (runnerExtension.includes(forbidden)) {
    fail(`${runnerExtensionPath} must keep host interruption separate from user cancellation/failure: ${forbidden}`);
  }
}

requireMarkers('crates/rustok-index/src/infrastructure/postgres/mod.rs', [
  'mod source_replay_runner {',
  'include!("source_replay_runner.rs");',
  'mod graceful_shutdown;',
]);

requireMarkers('crates/rustok-index/docs/m6-cooperative-page-interruption.md', [
  'Status: `worker_and_runner_source_complete_host_binding_pending`',
  '`run_next_page_interruptible`',
  '`PostgresIndexReplayRunner::run_interruptible`',
  '`IndexReplayError::Interrupted`',
  '`IndexReplayError::InterruptionCheckFailed`',
  'before every mutation',
  'never commits the page checkpoint',
  '30-second source-call timeout wrapper',
  'bounded replay dry-run',
  'does not impose a deadline on the probe future itself',
  'server lifecycle still does not supply its `StopHandle`',
  'runner interruption after durable mutation / before checkpoint commit',
]);
requireMarkers('crates/rustok-index/docs/README.md', [
  '[M6 Cooperative Replay-page Interruption](./m6-cooperative-page-interruption.md)',
]);
requireMarkers('crates/rustok-index/docs/implementation-plan.md', [
  '- [ ] Add in-page interruption/timeouts, dry-run, and targeted/full/shadow rebuild modes.',
]);
requireMarkers('scripts/verify/verify-index-query-contract.mjs', [
  "'verify-index-replay-page-interruption.mjs'",
  "'verify-index-replay-graceful-shutdown.mjs'",
]);

console.log('[verify-index-replay-page-interruption] worker safe points remain storage-neutral while the separate runner extension retains lease-aware host interruption; server StopHandle binding remains open');

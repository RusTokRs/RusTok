#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const read = (relative) => fs.readFileSync(path.join(root, relative), 'utf8');
const fail = (message) => {
  console.error(`[verify-index-replay-graceful-shutdown] ${message}`);
  process.exit(1);
};
const requireMarkers = (relative, markers) => {
  const source = read(relative);
  for (const marker of markers) {
    if (!source.includes(marker)) fail(`${relative} is missing ${marker}`);
  }
  return source;
};

const workerPath = 'crates/rustok-index/src/application/source_replay.rs';
const worker = requireMarkers(workerPath, [
  'pub async fn run_next_page_interruptible<Check, CheckFuture>(',
  'check_replay_interruption(&mut should_interrupt).await?;',
  'before every\n    /// mutation application, and before checkpoint commit'.replace('\\n', '\n'),
  'IndexReplayError::Interrupted',
]);
if (worker.includes('cancel_requested')) {
  fail(`${workerPath} generic one-page interruption must remain independent of persisted cancellation`);
}

const extensionPath = 'crates/rustok-index/src/infrastructure/postgres/source_replay_runner/graceful_shutdown.rs';
const extension = requireMarkers(extensionPath, [
  'pub async fn run_interruptible<Check>(',
  'Check: FnMut() -> bool',
  '.run_next_page_interruptible(request.page_request().clone(), || {',
  'Ok::<bool, crate::IndexReplayFailure>(interrupted)',
  'Err(crate::IndexReplayError::Interrupted) => {',
  'yield_after_host_interruption(&self.db, &lease, aggregate).await',
  'if cancel_if_requested(db, lease).await?',
  'match yield_for_resume(db, lease).await?',
  'aggregate.status = IndexReplayRunStatus::Yielded;',
  'aggregate.status = IndexReplayRunStatus::Cancelled;',
]);
for (const forbidden of [
  'request_cancel(',
  'cancel_requested = TRUE',
  'finish_failure(db, lease',
  'IndexReplayRunStatus::Complete;',
]) {
  if (extension.includes(forbidden)) {
    fail(`${extensionPath} must yield host interruption without manufacturing cancellation/failure: ${forbidden}`);
  }
}
const interrupted = extension.indexOf('Err(crate::IndexReplayError::Interrupted) => {');
const helper = extension.indexOf('async fn yield_after_host_interruption(');
const cancel = extension.indexOf('if cancel_if_requested(db, lease).await?', helper);
const yieldCall = extension.indexOf('match yield_for_resume(db, lease).await?', cancel);
if (interrupted < 0 || helper < 0 || cancel <= helper || yieldCall <= cancel) {
  fail('host interruption must preserve a persisted cancel race before yielding the lease to pending');
}

const ordinaryPath = 'crates/rustok-index/src/infrastructure/postgres/source_replay_runner.rs';
const ordinary = requireMarkers(ordinaryPath, [
  'pub async fn run(',
  'worker.run_next_page(request.page_request().clone())',
  'pub async fn request_cancel(',
  'yield_for_resume(&self.db, &lease).await?',
]);
if (ordinary.includes('run_interruptible<Check>')) {
  fail(`${ordinaryPath} ordinary runner file must remain unchanged by the host-probe extension`);
}

requireMarkers('crates/rustok-index/src/infrastructure/postgres/mod.rs', [
  'mod source_replay_runner {',
  'include!("source_replay_runner.rs");',
  'mod graceful_shutdown;',
  'mod source_replay_graceful_shutdown_tests;',
]);

const packetPath = 'crates/rustok-index/src/infrastructure/postgres/source_replay_graceful_shutdown_tests.rs';
const packet = requireMarkers(packetPath, [
  'host_stop_before_scan_yields_pending_and_restart_completes_with_new_attempt',
  'host_stop_after_durable_mutation_before_checkpoint_replays_as_duplicate_on_restart',
  '.run_interruptible(fixture.request("worker-a"), || true)',
  'probe_calls.fetch_add(1, Ordering::SeqCst) + 1 >= 3',
  'assert_eq!(probe_calls.load(Ordering::SeqCst), 3);',
  'IndexReplayRunStatus::Yielded',
  'state = \'pending\'',
  'lease_owner IS NULL',
  'SELECT COUNT(*) AS value FROM index_checkpoints',
  'resumed.attempt_count(), Some(2)',
  'resumed.duplicate_count(), 1',
  'state = \'succeeded\'',
]);
for (const forbidden of [
  'tokio::time::sleep',
  'tokio::spawn',
  'DbBackend::Postgres',
  'postgres://',
  'postgresql://',
  'request_cancel(',
]) {
  if (packet.includes(forbidden)) {
    fail(`${packetPath} must remain deterministic SQLite restart/redelivery source evidence: ${forbidden}`);
  }
}

requireMarkers('crates/rustok-index/docs/m6-replay-graceful-shutdown.md', [
  'Status: `runner_source_complete_host_binding_execution_pending`.',
  '`PostgresIndexReplayRunner::run_interruptible`',
  'Host interruption is not user cancellation',
  'pending',
  '`Duplicate`',
  'does **not** yet connect a server `StopHandle`',
]);

console.log('[verify-index-replay-graceful-shutdown] interruptible runner yields host stops to pending and retains duplicate-safe restart source evidence; server StopHandle binding remains open');

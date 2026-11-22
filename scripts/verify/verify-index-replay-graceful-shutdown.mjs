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

const workerPath = 'crates/modules/rustok-index/src/application/source_replay.rs';
const worker = requireMarkers(workerPath, [
  'pub async fn run_next_page_interruptible<Check, CheckFuture>(',
  'check_replay_interruption(&mut should_interrupt).await?;',
  'before every\n    /// mutation application, and before checkpoint commit'.replace('\\n', '\n'),
  'IndexReplayError::Interrupted',
]);
if (worker.includes('cancel_requested') || worker.includes('StopHandle')) {
  fail(`${workerPath} generic one-page interruption must remain independent of persisted cancellation/server lifecycle`);
}

const runnerPath = 'crates/modules/rustok-index/src/infrastructure/postgres/source_replay_runner.rs';
const runner = requireMarkers(runnerPath, [
  'pub async fn run_interruptible<Check>(',
  'self.run_interruptible(request, || false).await',
  'Check: FnMut() -> bool',
  'worker.run_next_page_interruptible(',
  'Ok::<bool, crate::IndexReplayFailure>(interrupted)',
  'drive_page(',
  'Err(IndexReplayError::Interrupted) => break,',
  'match yield_for_resume(&self.db, &lease).await?',
]);
const interrupted = runner.indexOf('Err(IndexReplayError::Interrupted) => break,');
const retry = runner.indexOf('store.record_failure(&lease, &failure).await', interrupted);
const yieldCall = runner.indexOf('match yield_for_resume(&self.db, &lease).await?', interrupted);
const finish = runner.indexOf('async fn terminal_write_outcome(');
const cancel = runner.indexOf('if cancel_if_requested(db, lease).await?', finish);
if (interrupted < 0 || retry <= interrupted || yieldCall <= retry || finish < 0 || cancel <= finish) {
  fail('host interruption must bypass failure accounting and preserve a persisted cancel race on yield');
}
if (runner.includes('StopHandle')) fail(`${runnerPath} must be host lifecycle-type neutral`);

requireMarkers('crates/modules/rustok-index/src/infrastructure/postgres/replay_runtime.rs', [
  'pub async fn run_operator<Check>(',
  'authority.require_tenant(request.page_request().tenant_id())?;',
  '.run_interruptible(request, should_interrupt)',
]);
const operatorPath = 'apps/server/src/services/index_replay_runtime_composition.rs';
const operator = requireMarkers(operatorPath, [
  'pub async fn run_interruptible<Check>(',
  'context.authorize_for(request.page_request().tenant_id())?;',
  '.run_operator(authority, request, should_interrupt)',
]);
if (operator.includes('StopHandle')) {
  fail(`${operatorPath} must remain lifecycle-type neutral`);
}

const schemaInitPath = 'apps/server/src/services/graphql_schema.rs';
const schemaInit = requireMarkers(schemaInitPath, [
  'use crate::services::app_lifecycle::StopHandle;',
  'let stop_handle = stop_handle_from_context(ctx);',
  'let handle = StopHandle::ensure(ctx);',
  'IndexReplayStopKeepalive',
  '_receiver: handle.subscribe()',
  'Keeps at least one watch receiver alive for API-only hosts',
  'stop_handle,',
]);
if (schemaInit.includes('stop_handle.stop()')) {
  fail(`${schemaInitPath} schema composition must never trigger shutdown`);
}
requireMarkers('apps/server/src/graphql/schema.rs', [
  'pub stop_handle: StopHandle,',
  '.data(stop_handle)',
]);
const transportPath = 'apps/server/src/graphql/index_replay.rs';
const transport = requireMarkers(transportPath, [
  'let stop_handle = ctx.data::<StopHandle>()?.clone();',
  '.run_interruptible(operator_context, request, || stop_handle.is_stopping())',
  '.request_cancel(operator_context, job_id)',
]);
if (transport.includes('stop_handle.stop()')) {
  fail(`${transportPath} transport may observe but must never trigger server shutdown`);
}
const runInputStart = transport.indexOf('pub struct IndexReplayRunInput');
const runInputEnd = transport.indexOf('\n}', runInputStart);
const runInput = transport.slice(runInputStart, runInputEnd);
for (const forbidden of ['stop', 'shutdown', 'probe', 'StopHandle']) {
  if (runInput.includes(forbidden)) fail(`GraphQL replay input exposes lifecycle marker ${forbidden}`);
}

requireMarkers('crates/modules/rustok-index/src/infrastructure/postgres/mod.rs', [
  'mod source_replay_runner;',
  'mod source_replay_graceful_shutdown_tests;',
]);

const packetPath = 'crates/modules/rustok-index/src/infrastructure/postgres/source_replay_graceful_shutdown_tests.rs';
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

requireMarkers('crates/modules/rustok-index/docs/m6-replay-graceful-shutdown.md', [
  'Status: `host_binding_source_complete_execution_pending`.',
  '`PostgresIndexReplayRunner::run_interruptible`',
  '`SharedIndexReplayRuntime::run_interruptible`',
  '`IndexReplayOperatorRuntime::run_interruptible`',
  '`StopHandle::is_stopping`',
  'Host interruption is not user cancellation',
  'pending',
  '`Duplicate`',
  'actual server-shutdown path have not been executed',
]);

console.log('[verify-index-replay-graceful-shutdown] server-owned StopHandle observation remains pending/duplicate-safe while interruptible pages share the ordinary in-page lease-maintenance policy');

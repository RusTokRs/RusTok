#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const read = (relative) => fs.readFileSync(path.join(root, relative), 'utf8');
const fail = (message) => {
  console.error(`[verify-index-replay-graphql-shutdown-evidence] ${message}`);
  process.exit(1);
};
const requireMarkers = (relative, markers) => {
  const source = read(relative);
  for (const marker of markers) {
    if (!source.includes(marker)) fail(`${relative} is missing ${marker}`);
  }
  return source;
};

const packetPath = 'apps/server/src/graphql/index_replay_shutdown_tests.rs';
const packet = requireMarkers(packetPath, [
  'use super::index_replay::IndexReplayMutation;',
  'materialize_index_replay_runtime',
  'PostgresSchemaRegistrationStore',
  'Schema<ReplayTestQuery, IndexReplayMutation, EmptySubscription>',
  '.data(extensions)',
  '.data(stop_handle.clone())',
  'with_rbac_request_scope(',
  'Permission::MODULES_MANAGE',
  'tokio::spawn(async move {',
  'gate.started.notified().await;',
  'first.stop_handle.stop().await;',
  'gate.release.notify_one();',
  'first_run["status"].as_str(), Some("YIELDED")',
  'job_state(&db).await, "pending"',
  'job_attempt_count(&db).await, 1',
  'pending_uncancelled_lease_free_jobs(&db).await, 1',
  'checkpoint_count(&db).await, 0',
  'materialized_entity_count(&db).await, 0',
  'applied_inbox_count(&db).await, 0',
  'let restarted = graphql_runtime(&db, None).await;',
  'second_run["status"].as_str(), Some("COMPLETE")',
  'job_attempt_count(&db).await, 2',
  'job_state(&db).await, "succeeded"',
  'checkpoint_count(&db).await, 1',
  'materialized_entity_count(&db).await, 1',
  'applied_inbox_count(&db).await, 1',
  'ScanGate',
  'Arc<Notify>',
  'gate.started.notify_one();',
  'gate.release.notified().await;',
]);

for (const forbidden of [
  'tokio::time::sleep',
  'std::thread::sleep',
  'interval(',
  'sleep_until(',
  'while !',
  'loop {',
  'request_cancel(',
  'cancelIndexReplay',
  'run_interruptible(',
  'PostgresIndexReplayRunner',
]) {
  if (packet.includes(forbidden)) {
    fail(`${packetPath} must drive shutdown through the real GraphQL command without timing polling or bypassing the guarded operator: ${forbidden}`);
  }
}

const spawn = packet.indexOf('tokio::spawn(async move {');
const scope = packet.indexOf('with_rbac_request_scope(', spawn);
const execute = packet.indexOf('first_schema.execute(replay_request())', scope);
const started = packet.indexOf('gate.started.notified().await;', execute);
const stop = packet.indexOf('first.stop_handle.stop().await;', started);
const release = packet.indexOf('gate.release.notify_one();', stop);
if (spawn < 0 || scope <= spawn || execute <= scope || started <= execute || stop <= started || release <= stop) {
  fail('first GraphQL request must install request authority inside its task, enter source scan, publish stop, then release scan deterministically');
}

const firstYield = packet.indexOf('Some("YIELDED")', release);
const pending = packet.indexOf('job_state(&db).await, "pending"', firstYield);
const restart = packet.indexOf('let restarted = graphql_runtime(&db, None).await;', pending);
const secondExecute = packet.indexOf('restarted.schema.execute(replay_request())', restart);
const complete = packet.indexOf('Some("COMPLETE")', secondExecute);
const attemptTwo = packet.indexOf('job_attempt_count(&db).await, 2', complete);
if (firstYield < 0 || pending <= firstYield || restart <= pending || secondExecute <= restart || complete <= secondExecute || attemptTwo <= complete) {
  fail('packet must retain yielded pending state before fresh GraphQL/runtime composition resumes and completes attempt 2');
}

requireMarkers('apps/server/src/graphql/mod.rs', [
  '#[cfg(test)]\nmod index_replay_shutdown_tests;'.replace('\\n', '\n'),
]);
requireMarkers('apps/server/src/graphql/index_replay.rs', [
  'let stop_handle = ctx.data::<StopHandle>()?.clone();',
  '.run_interruptible(operator_context, request, || stop_handle.is_stopping())',
]);
requireMarkers('apps/server/src/services/index_replay_runtime_composition.rs', [
  'pub async fn run_interruptible<Check>(',
  'context.authorize_for(request.page_request().tenant_id())?;',
  '.run_interruptible(request, should_interrupt)',
]);
requireMarkers('crates/rustok-index/src/infrastructure/postgres/replay_runtime.rs', [
  'pub async fn run_interruptible<Check>(',
  '.run_interruptible(request, should_interrupt)',
]);
requireMarkers('crates/rustok-index/src/infrastructure/postgres/source_replay_runner/graceful_shutdown.rs', [
  'Err(crate::IndexReplayError::Interrupted) => {',
  'yield_after_host_interruption(&self.db, &lease, aggregate).await',
  'match yield_for_resume(db, lease).await?',
]);
requireMarkers('apps/server/src/services/graphql_schema.rs', [
  'let stop_handle = stop_handle_from_context(ctx);',
  'IndexReplayStopKeepalive',
  'avoid a zero-receiver window',
]);

requireMarkers('apps/server/docs/index-replay-graphql-shutdown-evidence.md', [
  'Status: `source_complete_execution_pending`.',
  'real `IndexReplayMutation`',
  '`Notify`',
  '`StopHandle::stop()`',
  '`YIELDED`',
  '`pending`',
  'fresh runtime/GraphQL composition',
  'attempt `2`',
  'runner-level graceful-shutdown packet',
  'does not claim full HTTP/process bootstrap execution',
]);

console.log('[verify-index-replay-graphql-shutdown-evidence] deterministic schema-data GraphQL shutdown packet retains authorized stop-to-pending and fresh-runtime attempt-2 completion without sleeps or polling');

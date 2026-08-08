#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const read = (relative) => fs.readFileSync(path.join(root, relative), 'utf8');
const fail = (message) => {
  console.error(`[verify-index-replay-shadow-host-dispatch] ${message}`);
  process.exit(1);
};
const requireMarkers = (relative, markers) => {
  const source = read(relative);
  for (const marker of markers) {
    if (!source.includes(marker)) fail(`${relative} is missing ${marker}`);
  }
  return source;
};

const serverPath = 'apps/server/src/services/index_replay_runtime_composition.rs';
const server = requireMarkers(serverPath, [
  'Shadow(#[from] rustok_index::IndexReplayDryRunError)',
  'shadow: rustok_index::SharedIndexReplayDryRunRuntime',
  'pub async fn run_shadow(',
  'request: rustok_index::IndexReplayDryRunRequest',
  'context.authorize_for(request.tenant_id())?;',
  'self.shadow.run(request).await.map_err(Into::into)',
  '.get::<rustok_index::SharedIndexReplayDryRunRuntime>()',
  'IndexReplayOperatorRuntime::new(runtime, shadow)',
  'shadow_dispatch_reuses_request_bound_modules_manage_guard',
  'vec![Permission::MODULES_READ]',
  'IndexReplayOperatorError::Forbidden',
  'vec![Permission::MODULES_MANAGE]',
  'IndexReplayDryRunStatus::Complete',
]);
for (const forbidden of ['tokio::spawn', 'tokio::time::sleep', 'loop {']) {
  if (server.includes(forbidden)) fail(`${serverPath} must not create a Shadow scheduler/lifecycle owner: ${forbidden}`);
}

const graphqlPath = 'apps/server/src/graphql/index_replay.rs';
const graphql = requireMarkers(graphqlPath, [
  'async fn run_index_replay(',
  '.run_interruptible(operator_context, request, || stop_handle.is_stopping())',
  'async fn cancel_index_replay(',
  'prepare_authorized_run(',
]);
for (const forbidden of [
  'run_index_replay_shadow',
  '.run_shadow(',
  'IndexReplayDryRunRequest',
  'SharedIndexReplayDryRunRuntime',
]) {
  if (graphql.includes(forbidden)) {
    fail(`${graphqlPath} must remain Full/cancel-only until the separate Shadow transport slice: ${forbidden}`);
  }
}

const runnerPath = 'crates/rustok-index/src/infrastructure/postgres/source_replay_runner.rs';
const runner = read(runnerPath);
for (const forbidden of ['IndexReplayMode::Shadow', 'SideEffectFreeScan', 'SharedIndexReplayDryRunRuntime']) {
  if (runner.includes(forbidden)) {
    fail(`${runnerPath} must remain the durable Full replay runner: ${forbidden}`);
  }
}

requireMarkers('crates/rustok-index/docs/m6-bounded-replay-dry-run.md', [
  'Status: `source_complete_transport_pending`',
  '`IndexReplayOperatorRuntime::run_shadow`',
  'same request-bound `modules:manage` authorization boundary',
  'GraphQL, HTTP, CLI, or admin transport surfaces',
  'no durable replay job or checkpoint',
]);
requireMarkers('crates/rustok-index/docs/m6-replay-mode-contract.md', [
  'Status: `source_complete_shadow_host_dispatch_transport_pending`.',
  '`Shadow` host dispatch is now source-complete',
  '`IndexReplayOperatorRuntime::run_shadow`',
  'GraphQL transport remains separate',
]);
requireMarkers('crates/rustok-index/docs/implementation-plan-current-2026-08-08.md', [
  'Guard the existing side-effect-free Shadow replay runtime behind the request-bound `modules:manage` operator boundary.',
  'Add authorization-first GraphQL transport for the guarded Shadow replay command.',
  'Targeted execution remains separate until a bounded mutation-application contract over `IndexSource::load` exists.',
]);

console.log('[verify-index-replay-shadow-host-dispatch] Shadow replay is request-bound and modules:manage-guarded without changing Full durable ownership or exposing transport');

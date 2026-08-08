#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const read = (relative) => fs.readFileSync(path.join(root, relative), 'utf8');
const fail = (message) => {
  console.error(`[verify-index-replay-targeted-host-dispatch] ${message}`);
  process.exit(1);
};
const requireMarkers = (relative, markers) => {
  const source = read(relative);
  for (const marker of markers) {
    if (!source.includes(marker)) fail(`${relative} is missing ${marker}`);
  }
  return source;
};

const runtimePath = 'crates/rustok-index/src/infrastructure/postgres/replay_runtime.rs';
const runtime = requireMarkers(runtimePath, [
  'targeted: Arc<IndexReplayTargetedExecutor<PostgresMutationStore>>',
  'pub async fn run_targeted(',
  'request: IndexSourceLoadRequest',
  '.run(crate::IndexReplayModeSelection::Targeted(request))',
  'let targeted = IndexReplayTargetedExecutor::new(',
  'sources.clone(),',
  'schema_registry.clone(),',
  'PostgresMutationStore::new(db.clone())',
  'PostgresIndexReplayRunner::new(db, sources, schema_registry)',
]);
for (const forbidden of [
  'IndexReplayMode::Targeted',
  'PostgresIndexReplayJobStore',
  'PostgresIndexReplayCheckpointStore',
  'request_cancel(request',
  'tokio::spawn',
  'tokio::time::sleep',
  'loop {',
  'partition_key',
]) {
  if (runtime.includes(forbidden)) {
    fail(`${runtimePath} must compose Targeted without a second durable/lifecycle owner: ${forbidden}`);
  }
}
const targetedBuild = runtime.indexOf('let targeted = IndexReplayTargetedExecutor::new(');
const runtimeBuild = runtime.indexOf('let runtime = SharedIndexReplayRuntime::new(', targetedBuild);
if (targetedBuild < 0 || runtimeBuild <= targetedBuild) {
  fail('Targeted executor must be assembled before the shared replay runtime is published');
}

const serverPath = 'apps/server/src/services/index_replay_runtime_composition.rs';
const server = requireMarkers(serverPath, [
  'Targeted(#[from] rustok_index::IndexReplayTargetedError)',
  'pub async fn run_targeted(',
  'request: rustok_index::IndexSourceLoadRequest',
  'context.authorize_for(request.tenant_id())?;',
  'self.inner.run_targeted(request).await.map_err(Into::into)',
  'targeted_dispatch_reuses_request_bound_modules_manage_guard',
  'vec![Permission::MODULES_READ]',
  'vec![Permission::MODULES_MANAGE]',
  'assert_eq!(outcome.requested_count(), 1);',
  'assert_eq!(outcome.missing_count(), 1);',
]);
const targetedMethod = server.indexOf('pub async fn run_targeted(');
const authorize = server.indexOf('context.authorize_for(request.tenant_id())?;', targetedMethod);
const delegate = server.indexOf('self.inner.run_targeted(request).await.map_err(Into::into)', authorize);
if (targetedMethod < 0 || authorize <= targetedMethod || delegate <= authorize) {
  fail('Targeted host dispatch must remain authorize exact tenant/modules:manage before runtime execution');
}
for (const forbidden of [
  'targeted_worker',
  'targeted_checkpoint',
  'targeted_job',
  'targeted_lease',
  'targeted_cancel',
  'targeted_retry',
  'targeted_scheduler',
]) {
  if (server.includes(forbidden)) {
    fail(`${serverPath} gained a forbidden Targeted ownership surface: ${forbidden}`);
  }
}

const appPath = 'crates/rustok-index/src/application/targeted_replay.rs';
requireMarkers(appPath, [
  'IndexReplayModeSelection::Targeted(request) => request',
  'for (position, key) in request.keys().iter().enumerate()',
  'source_for_schema(request.schema())',
  '.load(request)',
  'let mut event_ids = BTreeSet::<Uuid>::new();',
  '.apply_replay_mutation(self.schemas.as_ref(), &source_name, mutation)',
  'targeted_exact_retry_replays_stable_event_ids_after_partial_failure',
]);

const graphqlPath = 'apps/server/src/graphql/index_replay.rs';
const graphqlProduction = read(graphqlPath).split('\n#[cfg(test)]')[0];
for (const forbidden of [
  'run_index_replay_targeted',
  'runIndexReplayTargeted',
  '.run_targeted(',
  'IndexReplayTargetedOutcome',
]) {
  if (graphqlProduction.includes(forbidden)) {
    fail(`${graphqlPath} must not expose Targeted publicly in the host-dispatch slice: ${forbidden}`);
  }
}

requireMarkers('crates/rustok-index/docs/m6-targeted-replay-mutation-application.md', [
  'Status: `source_complete_host_guard_transport_pending`.',
  '## PostgreSQL/runtime composition',
  '`IndexReplayTargetedExecutor<PostgresMutationStore>`',
  '`IndexReplayOperatorRuntime` owns Targeted dispatch',
  'dedicated authorization-first Targeted public transport',
]);
requireMarkers('crates/rustok-index/docs/m6-replay-mode-contract.md', [
  'Status: `source_complete_targeted_host_guard_transport_pending`.',
  '## Targeted PostgreSQL composition and host dispatch',
  '`run_targeted(IndexSourceLoadRequest)`',
  'same effective `modules:manage`',
  'dedicated authorization-first public transport',
]);
requireMarkers('crates/rustok-index/docs/implementation-plan-current-2026-08-08.md', [
  'Materialize the bounded Targeted replay executor with `PostgresMutationStore` and guard host dispatch behind request-bound `modules:manage`.',
  'Add a dedicated authorization-first Targeted GraphQL transport over `IndexReplayOperatorRuntime::run_targeted`.',
]);

console.log('[verify-index-replay-targeted-host-dispatch] Targeted uses the canonical PostgreSQL mutation sink and exact request-bound modules:manage host guard without Full durable ownership or public transport');

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
  'pub enum IndexReplayTargetedOperatorError',
  'Authorization(#[from] IndexReplayOperatorError)',
  'Targeted(#[from] rustok_index::IndexReplayTargetedError)',
  'pub async fn run_targeted(',
  'request: rustok_index::IndexSourceLoadRequest',
  'Result<rustok_index::IndexReplayTargetedOutcome, IndexReplayTargetedOperatorError>',
  'context.authorize_for(request.tenant_id())?;',
  'self.inner.run_targeted(request).await.map_err(Into::into)',
  'targeted_dispatch_reuses_request_bound_modules_manage_guard',
  'vec![Permission::MODULES_READ]',
  'IndexReplayTargetedOperatorError::Authorization(IndexReplayOperatorError::Forbidden)',
  'vec![Permission::MODULES_MANAGE]',
  'assert_eq!(outcome.requested_count(), 1);',
  'assert_eq!(outcome.missing_count(), 1);',
]);
const fullErrorStart = server.indexOf('pub enum IndexReplayOperatorError');
const targetedErrorStart = server.indexOf('pub enum IndexReplayTargetedOperatorError', fullErrorStart);
const fullError = server.slice(fullErrorStart, targetedErrorStart);
if (fullError.includes('IndexReplayTargetedError') || fullError.includes('Targeted(')) {
  fail('Targeted failures must not widen the existing Full/cancel IndexReplayOperatorError surface');
}
const targetedMethod = server.indexOf('pub async fn run_targeted(');
const authorize = server.indexOf('context.authorize_for(request.tenant_id())?;', targetedMethod);
const delegate = server.indexOf('self.inner.run_targeted(request).await.map_err(Into::into)', authorize);
if (targetedMethod < 0 || authorize <= targetedMethod || delegate <= authorize) {
  fail('Targeted host dispatch must authorize exact tenant/modules:manage before runtime execution');
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
const graphql = requireMarkers(graphqlPath, [
  'pub struct IndexReplayTargetedRunInput',
  'pub struct IndexReplayTargetedKeyInput',
  'async fn run_index_replay_targeted(',
  'prepare_authorized_targeted_run(tenant.id, auth.user_id, input)',
  '.run_targeted(operator_context, request)',
  '.map_err(map_targeted_operator_error)?',
  'IndexReplayTargetedOperatorError::Authorization(error) => map_operator_error(error)',
  'IndexReplayTargetedOperatorError::Targeted(error) => map_targeted_error(error)',
]);
const production = graphql.split('\n#[cfg(test)]')[0];
for (const forbidden of [
  'SharedIndexReplayRuntime',
  'IndexReplayTargetedExecutor',
  'PostgresMutationStore',
  'PostgresIndexReplayRunner',
  'DatabaseConnection',
]) {
  if (production.includes(forbidden)) {
    fail(`${graphqlPath} must route Targeted only through the guarded server operator: ${forbidden}`);
  }
}

requireMarkers('crates/rustok-index/docs/m6-targeted-replay-mutation-application.md', [
  'Status: `source_complete_transport_execution_pending`.',
  '## PostgreSQL/runtime composition',
  '## GraphQL transport',
  '`runIndexReplayTargeted(input: ...)`',
  'does not expose it',
]);
requireMarkers('crates/rustok-index/docs/m6-replay-runtime-composition.md', [
  'Targeted and Shadow each keep a separate typed operator error wrapper',
  'does not widen the existing GraphQL Full/cancel error contract',
  '`runIndexReplayTargeted` is mounted on the existing `IndexReplayMutation` object',
]);
requireMarkers('crates/rustok-index/docs/m6-replay-mode-contract.md', [
  'Status: `source_complete_targeted_graphql_execution_pending`.',
  '## Targeted PostgreSQL composition and host dispatch',
  '## Targeted GraphQL transport',
  '`run_targeted(IndexSourceLoadRequest)`',
  'same effective `modules:manage`',
]);
requireMarkers('crates/rustok-index/docs/implementation-plan-current-2026-08-08.md', [
  'Materialize the bounded Targeted replay executor with `PostgresMutationStore` and guard host dispatch behind request-bound `modules:manage`.',
  'Add a dedicated authorization-first Targeted GraphQL transport over `IndexReplayOperatorRuntime::run_targeted`.',
]);

console.log('[verify-index-replay-targeted-host-dispatch] Targeted uses the canonical PostgreSQL mutation sink and exact request-bound modules:manage host guard, and GraphQL delegates only through that isolated operator surface');

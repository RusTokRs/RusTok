#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const read = (relative) => fs.readFileSync(path.join(root, relative), 'utf8');
const fail = (message) => {
  console.error(`[verify-index-replay-mode-contract] ${message}`);
  process.exit(1);
};
const requireMarkers = (relative, markers) => {
  const source = read(relative);
  for (const marker of markers) {
    if (!source.includes(marker)) fail(`${relative} is missing ${marker}`);
  }
  return source;
};

const modePath = 'crates/rustok-index/src/application/replay_mode.rs';
const mode = requireMarkers(modePath, [
  'pub enum IndexReplayMode {',
  'Full,',
  'Targeted,',
  'Shadow,',
  'pub enum IndexReplayExecutionSurface {',
  'DurableScan,',
  'TargetedLoad,',
  'SideEffectFreeScan,',
  'pub enum IndexReplayModeSelection {',
  'Targeted(IndexSourceLoadRequest)',
  'IndexSourceLoadRequest::new(keys)?',
  'IndexReplayExecutionSurface::DurableScan',
  'IndexReplayExecutionSurface::TargetedLoad',
  'IndexReplayExecutionSurface::SideEffectFreeScan',
  'pub fn is_admitted_to_durable_scan_runner(&self) -> bool',
  'matches!(self, Self::Full)',
  'targeted_reuses_the_canonical_bounded_load_request',
  'targeted_rejects_empty_duplicate_and_mixed_scope_keys',
  'shadow_routes_only_to_the_side_effect_free_scan_surface',
]);
for (const forbidden of [
  'PostgresIndexReplayJobStore',
  'PostgresIndexReplayCheckpointStore',
  'PostgresMutationStore',
  'DatabaseConnection',
  'partition_key',
  'request_cancel',
  'Retrying',
  'auto_requeue',
]) {
  if (mode.includes(forbidden)) fail(`${modePath} must remain an application mode/routing contract: ${forbidden}`);
}

requireMarkers('crates/rustok-index/src/application/mod.rs', [
  'mod replay_mode;',
  'mod targeted_replay;',
  'IndexReplayExecutionSurface, IndexReplayMode, IndexReplayModeSelection',
  'IndexReplayTargetedError, IndexReplayTargetedExecutor, IndexReplayTargetedOutcome',
]);

const targetedPath = 'crates/rustok-index/src/application/targeted_replay.rs';
const targeted = requireMarkers(targetedPath, [
  'pub struct IndexReplayTargetedExecutor<M>',
  'IndexReplayModeSelection::Targeted(request) => request',
  'for (position, key) in request.keys().iter().enumerate()',
  '.load(request)',
  'let mut event_ids = BTreeSet::<Uuid>::new();',
  'self.schemas.validate_mutation(mutation)',
  '.apply_replay_mutation(self.schemas.as_ref(), &source_name, mutation)',
  'missing_count: requested_count - mutation_count',
]);
for (const forbidden of [
  'PostgresIndexReplayJobStore',
  'PostgresIndexReplayCheckpointStore',
  'DatabaseConnection',
  'request_cancel(',
  'tokio::spawn',
  'partition_key',
]) {
  if (targeted.includes(forbidden)) {
    fail(`${targetedPath} must stay bounded and independent of durable Full ownership: ${forbidden}`);
  }
}

const runtimePath = 'crates/rustok-index/src/infrastructure/postgres/replay_runtime.rs';
const runtime = requireMarkers(runtimePath, [
  'targeted: Arc<IndexReplayTargetedExecutor<PostgresMutationStore>>',
  'pub async fn run_targeted(',
  '.run(crate::IndexReplayModeSelection::Targeted(request))',
  'PostgresMutationStore::new(db.clone())',
]);
for (const forbidden of ['PostgresIndexReplayJobStore', 'PostgresIndexReplayCheckpointStore', 'partition_key']) {
  if (runtime.includes(forbidden)) {
    fail(`${runtimePath} must not route Targeted through Full durable ownership: ${forbidden}`);
  }
}

const runnerPath = 'crates/rustok-index/src/infrastructure/postgres/source_replay_runner.rs';
const runner = read(runnerPath);
for (const forbidden of [
  'IndexReplayMode::Targeted',
  'IndexReplayMode::Shadow',
  'IndexReplayTargetedExecutor',
  'TargetedLoad',
  'SideEffectFreeScan',
]) {
  if (runner.includes(forbidden)) {
    fail(`${runnerPath} must remain the durable Full-scan runner: ${forbidden}`);
  }
}

const dryRunPath = 'crates/rustok-index/src/replay_dry_run.rs';
requireMarkers(dryRunPath, [
  'No mutation, inbox delivery, job, checkpoint, or reconciliation progress is persisted',
  'pub struct SharedIndexReplayDryRunRuntime',
  'locale: Option<LocaleKey>',
  'IndexSourceScanRequest::for_locale(',
  'IndexReplayDryRunError::LocaleScopeUnsupported',
  '.scan(scan_request)',
]);

const continuationPath = 'crates/rustok-index/src/application/source_continuation.rs';
const continuation = requireMarkers(continuationPath, [
  'locale: Option<LocaleKey>',
  'pub fn for_locale(',
  'IndexSourceContinuationError::LocaleScopeMismatch',
]);
for (const forbidden of ['CONTINUATION_VERSION', 'ContinuationClaimsV1', 'ContinuationClaimsV2']) {
  if (continuation.includes(forbidden)) {
    fail(`${continuationPath} must keep one canonical unversioned envelope: ${forbidden}`);
  }
}

requireMarkers('apps/server/src/services/index_replay_runtime_composition.rs', [
  'pub async fn run_targeted(',
  'context.authorize_for(request.tenant_id())?;',
  'self.inner.run_targeted(request).await.map_err(Into::into)',
  'pub async fn run_shadow(',
  'shadow: rustok_index::SharedIndexReplayDryRunRuntime',
  'self.shadow.run(request).await.map_err(Into::into)',
  'IndexReplayShadowTransportRuntime',
]);
requireMarkers('apps/server/src/services/index_replay_shadow_transport.rs', [
  'locale: Option<rustok_index::LocaleKey>',
  'IndexSourceContinuationScope::for_locale(',
  'IndexReplayDryRunRequest::for_locale(',
]);
const graphqlPath = 'apps/server/src/graphql/index_replay.rs';
const graphql = requireMarkers(graphqlPath, [
  'pub struct IndexReplayTargetedRunInput',
  'async fn run_index_replay_targeted(',
  'prepare_authorized_targeted_run(tenant.id, auth.user_id, input)',
  '.run_targeted(operator_context, request)',
  'pub struct IndexReplayShadowRunInput',
  'pub locale: Option<String>',
  'async fn run_index_replay_shadow(',
  '.get::<IndexReplayShadowTransportRuntime>()',
]);
const production = graphql.split('\n#[cfg(test)]')[0];
for (const forbidden of [
  'SharedIndexReplayRuntime',
  'IndexReplayTargetedExecutor',
  'PostgresMutationStore',
]) {
  if (production.includes(forbidden)) {
    fail(`GraphQL must not bypass dedicated guarded mode surfaces: ${forbidden}`);
  }
}

requireMarkers('crates/rustok-index/docs/m6-replay-mode-contract.md', [
  'Status: `source_complete_targeted_graphql_execution_pending`.',
  '`Full` — cursor-based durable source scan',
  '`Targeted` — bounded exact-key source load',
  '`Shadow` — side-effect-free cursor scan',
  'returns true only for `Full`',
  '## Targeted mutation application',
  '## Targeted PostgreSQL composition and host dispatch',
  '## Targeted GraphQL transport',
  '`IndexReplayTargetedExecutor<PostgresMutationStore>`',
  'requires the same effective',
  '`modules:manage` permission snapshot used by Full',
  '`runIndexReplayTargeted` is a dedicated mutation',
  '`runIndexReplayShadow` remains a dedicated transport',
  'one current unversioned envelope',
  'No additional independent source-only M6 replay boundary is open',
]);

requireMarkers('crates/rustok-index/docs/implementation-plan-current-2026-08-08.md', [
  'Define explicit Full/Targeted/Shadow replay mode identity and fail-closed execution surfaces.',
  'Guard the existing side-effect-free Shadow replay runtime behind the request-bound `modules:manage` operator boundary.',
  'Add authorization-first schema-wide GraphQL transport for guarded Shadow replay with sealed caller-carried continuation.',
  'Make Shadow continuation identity locale-safe before exposing exact-locale Shadow GraphQL transport.',
  'Add exact-locale Shadow dry-run/runtime/GraphQL execution using the canonical locale-safe continuation scope.',
  'Define a bounded Targeted mutation-application contract over `IndexSource::load` without aliasing durable scan ownership.',
  'Materialize the bounded Targeted replay executor with `PostgresMutationStore` and guard host dispatch behind request-bound `modules:manage`.',
  'Add a dedicated authorization-first Targeted GraphQL transport over `IndexReplayOperatorRuntime::run_targeted`.',
  'Add partition replay scope only after a real partition-capable source contract exists.',
  'There is no remaining independent source-only M6 replay expansion justified by the current contract.',
]);

console.log('[verify-index-replay-mode-contract] Full stays durable, Targeted uses bounded exact-key PostgreSQL/guarded GraphQL execution without durable ownership, and Shadow remains no-write/sealed');

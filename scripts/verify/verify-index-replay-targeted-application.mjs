#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const read = (relative) => fs.readFileSync(path.join(root, relative), 'utf8');
const fail = (message) => {
  console.error(`[verify-index-replay-targeted-application] ${message}`);
  process.exit(1);
};
const requireMarkers = (relative, markers) => {
  const source = read(relative);
  for (const marker of markers) {
    if (!source.includes(marker)) fail(`${relative} is missing ${marker}`);
  }
  return source;
};

const targetedPath = 'crates/rustok-index/src/application/targeted_replay.rs';
const targeted = requireMarkers(targetedPath, [
  'pub struct IndexReplayTargetedOutcome',
  'pub struct IndexReplayTargetedExecutor<M>',
  'IndexReplayModeSelection::Targeted(request) => request',
  'IndexReplayTargetedError::WrongMode',
  'let registered = self',
  '.get(request.schema())',
  'for (position, key) in request.keys().iter().enumerate()',
  'key.entity_id.is_nil()',
  '(LocaleMode::Required, false)',
  '(LocaleMode::None, true)',
  'IndexReplayTargetedError::InvalidTarget',
  'source_for_schema(request.schema())',
  '.load(request)',
  'let mut event_ids = BTreeSet::<Uuid>::new();',
  'event_id.is_nil()',
  'IndexReplayTargetedError::DuplicateEventId',
  'self.schemas.validate_mutation(mutation)',
  '.apply_replay_mutation(self.schemas.as_ref(), &source_name, mutation)',
  'missing_count: requested_count - mutation_count',
  'targeted_load_applies_only_returned_mutations_and_reports_missing_keys',
  'targeted_rejects_other_modes_without_source_or_mutation_execution',
  'targeted_rejects_invalid_requested_entity_and_locale_scope_before_load',
  'targeted_preflights_nil_duplicate_and_schema_invalid_batches_before_writes',
  'targeted_exact_retry_replays_stable_event_ids_after_partial_failure',
  'RetryOnceSink::new(Uuid::from_u128(901))',
  'assert_eq!(retry.duplicate_count(), 1);',
]);
const modeSelection = targeted.indexOf('IndexReplayModeSelection::Targeted(request) => request');
const schemaAdmission = targeted.indexOf('let registered = self', modeSelection);
const keyAdmission = targeted.indexOf('for (position, key) in request.keys().iter().enumerate()', schemaAdmission);
const sourceResolution = targeted.indexOf('source_for_schema(request.schema())', keyAdmission);
const load = targeted.indexOf('.load(request)', sourceResolution);
const batchPreflight = targeted.indexOf('let mut event_ids = BTreeSet::<Uuid>::new();', load);
const mutationWrite = targeted.indexOf('.apply_replay_mutation(self.schemas.as_ref(), &source_name, mutation)', batchPreflight);
if (
  modeSelection < 0 ||
  schemaAdmission <= modeSelection ||
  keyAdmission <= schemaAdmission ||
  sourceResolution <= keyAdmission ||
  load <= sourceResolution ||
  batchPreflight <= load ||
  mutationWrite <= batchPreflight
) {
  fail('Targeted order must remain mode -> active schema/key admission -> source -> load -> full batch preflight -> mutation writes');
}
for (const forbidden of [
  'DatabaseConnection',
  'PostgresMutationStore',
  'PostgresIndexReplayRunner',
  'PostgresIndexReplayJobStore',
  'PostgresIndexReplayCheckpointStore',
  'index_jobs',
  'index_checkpoints',
  'request_cancel(',
  'tokio::spawn',
  'tokio::time::sleep',
  'partition_key',
  'worker_id',
  'lease_duration',
  'heartbeat',
]) {
  if (targeted.includes(forbidden)) {
    fail(`${targetedPath} must remain a bounded application executor without durable scan ownership: ${forbidden}`);
  }
}

const mode = requireMarkers('crates/rustok-index/src/application/replay_mode.rs', [
  'Targeted(IndexSourceLoadRequest)',
  'IndexSourceLoadRequest::new(keys)?',
  'IndexReplayExecutionSurface::TargetedLoad',
  'matches!(self, Self::Full)',
]);
if (mode.includes('PostgresMutationStore')) {
  fail('replay_mode.rs must remain storage-neutral');
}

requireMarkers('crates/rustok-index/src/application/source_registry.rs', [
  'const MAX_LOAD_KEYS: usize = 256;',
  'pub struct IndexSourceLoadRequest',
  'IndexSourceError::EmptyLoadKeys',
  'IndexSourceError::TooManyLoadKeys',
  'IndexSourceError::MixedLoadScope',
  'IndexSourceError::DuplicateLoadKey',
  'pub async fn load(',
  'validate_load_batch(&request, &batch)?;',
  'IndexSourceError::LoadMutationNotRequested',
  'IndexSourceError::DuplicateLoadMutationKey',
]);
requireMarkers('crates/rustok-index/src/application/mod.rs', [
  'mod targeted_replay;',
  'IndexReplayTargetedError, IndexReplayTargetedExecutor, IndexReplayTargetedOutcome',
]);

const runnerPath = 'crates/rustok-index/src/infrastructure/postgres/source_replay_runner.rs';
const runner = read(runnerPath);
for (const forbidden of [
  'IndexReplayTargetedExecutor',
  'IndexReplayMode::Targeted',
  'IndexReplayModeSelection::Targeted',
  'TargetedLoad',
]) {
  if (runner.includes(forbidden)) {
    fail(`${runnerPath} must stay the durable Full scan runner: ${forbidden}`);
  }
}

requireMarkers('crates/rustok-index/docs/m6-targeted-replay-mutation-application.md', [
  'Status: `source_complete_host_guard_pending`.',
  '`IndexReplayTargetedExecutor`',
  'requested key admission',
  'Missing requested keys',
  'does not infer deletion',
  'source-owned mutation event UUID',
  'does not add a checkpoint for partial progress',
  'PostgreSQL/runtime composition plus request-bound server host dispatch',
]);
requireMarkers('crates/rustok-index/docs/m6-replay-mode-contract.md', [
  'Status: `source_complete_targeted_application_host_guard_pending`.',
  '## Targeted mutation application',
  '`IndexReplayTargetedExecutor`',
  'requested keys against the active schema',
  'Missing requested keys are allowed',
  'PostgreSQL/runtime materialization and request-bound server host',
]);
requireMarkers('crates/rustok-index/docs/implementation-plan-current-2026-08-08.md', [
  'Define a bounded Targeted mutation-application contract over `IndexSource::load` without aliasing durable scan ownership.',
  'requested Targeted keys against the active schema before source load',
  'Materialize the bounded Targeted replay executor with `PostgresMutationStore` and guard host dispatch behind request-bound `modules:manage`.',
]);
requireMarkers('crates/rustok-index/docs/README.md', [
  '[M6 Targeted Replay Mutation Application](./m6-targeted-replay-mutation-application.md)',
]);
requireMarkers('scripts/verify/verify-index-query-contract.mjs', [
  "'verify-index-replay-targeted-application.mjs'",
]);

console.log('[verify-index-replay-targeted-application] Targeted validates exact keys before load, preflights the whole batch, and converges exact retry without Full job/checkpoint ownership');

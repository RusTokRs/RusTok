#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const read = (relative) => fs.readFileSync(path.join(root, relative), 'utf8');
const fail = (message) => {
  console.error(`[verify-index-source-replay-contract] ${message}`);
  process.exit(1);
};
const requireMarkers = (relative, markers) => {
  const source = read(relative);
  for (const marker of markers) {
    if (!source.includes(marker)) fail(`${relative} is missing ${marker}`);
  }
  return source;
};

const registryPath = 'crates/rustok-index/src/application/source_registry.rs';
const registry = requireMarkers(registryPath, [
  'pub trait IndexSource: Send + Sync',
  'async fn scan(',
  'async fn load(',
  'pub struct IndexSourceCatalog',
  'pub struct SharedIndexSourceRegistry',
  'pub struct IndexSourceCursor',
  "impl<'de> Deserialize<'de> for IndexSourceCursor",
  'Self::new(value).map_err(D::Error::custom)',
  'pub struct IndexSourceScanRequest',
  'pub struct IndexSourceLoadRequest',
  'pub struct IndexSourcePage',
  'pub struct IndexSourceLoadBatch',
  'pub enum IndexSourceFailureKind',
  'Retryable',
  'Permanent',
  'MAX_CURSOR_BYTES: usize = 8 * 1024',
  'MAX_SCAN_BATCH_SIZE: usize = 1_000',
  'MAX_LOAD_KEYS: usize = 256',
  'SchemaIdentitySourceConflict',
  'UnpublishedSourceSchema',
  'SourceSchemaOwnerMismatch',
  'EmptyScanContinuation',
  'ScanCursorDidNotAdvance',
  'LoadMutationNotRequested',
  'materialize_index_source_registry(',
  'pub fn register_index_source<S>(',
  'source_materialization_requires_exact_schema_owner',
  'schema_identity_cannot_move_between_replay_sources',
  'cursor_and_scan_limits_are_bounded',
  'serde_json::from_value::<IndexSourceCursor>',
  'targeted_load_is_one_bounded_tenant_schema_scope',
]);

for (const forbidden of [
  'DatabaseConnection',
  'PostgresMutationStore',
  'index_jobs',
  'index_checkpoints',
  'tokio::spawn',
  'SELECT ',
  'INSERT ',
  'UPDATE ',
  'DELETE ',
]) {
  if (registry.includes(forbidden)) {
    fail(`${registryPath} contains forbidden storage/worker marker ${forbidden}`);
  }
}

const workerPath = 'crates/rustok-index/src/application/source_replay.rs';
const worker = requireMarkers(workerPath, [
  'pub struct IndexReplayWorker',
  'pub async fn run_next_page(',
  'load_replay_checkpoint(&checkpoint_key)',
  'CheckpointIdentityMismatch',
  'IndexReplayPageStatus::AlreadyComplete',
  'let mut event_ids = BTreeSet::new();',
  'event_id.is_nil()',
  'DuplicateReplayEventId',
  '.apply_replay_mutation(',
  'last_delivery_id = Some(mutation.event_id().to_string())',
  '.commit_replay_checkpoint(&checkpoint)',
  'IndexReplayPageStatus::Complete',
  'IndexReplayPageStatus::Advanced',
  'CheckpointReadFailed',
  'MutationFailed',
  'CheckpointCommitFailed',
]);
for (const forbidden of [
  'DatabaseConnection',
  'PostgresMutationStore',
  'partition_key',
  'index_jobs',
  'tokio::spawn',
  'SELECT ',
  'INSERT ',
  'UPDATE ',
  'DELETE ',
]) {
  if (worker.includes(forbidden)) {
    fail(`${workerPath} contains forbidden infrastructure/scheduler marker ${forbidden}`);
  }
}
const runStart = worker.indexOf('pub async fn run_next_page(');
const validationPosition = worker.indexOf('let mut event_ids = BTreeSet::new();', runStart);
const applyPosition = worker.indexOf('.apply_replay_mutation(', runStart);
const commitPosition = worker.indexOf('.commit_replay_checkpoint(&checkpoint)', runStart);
if (
  runStart < 0
  || validationPosition < runStart
  || applyPosition <= validationPosition
  || commitPosition <= applyPosition
) {
  fail(`${workerPath} must validate the full page, apply mutations, then commit the checkpoint`);
}

const postgresPath = 'crates/rustok-index/src/infrastructure/postgres/source_replay.rs';
const postgres = requireMarkers(postgresPath, [
  'impl IndexReplayMutationSink for PostgresMutationStore',
  'MutationDelivery::from_event(source_name, mutation.clone())',
  'pub struct PostgresIndexReplayCheckpointStore',
  'lease: IndexReplayJobLease',
  'pub fn new(db: DatabaseConnection, lease: IndexReplayJobLease)',
  'impl IndexReplayCheckpointStore for PostgresIndexReplayCheckpointStore',
  'validate_checkpoint_identity(&self.lease, key)?;',
  'validate_checkpoint_identity(&self.lease, checkpoint.key())?;',
  'assert_active_replay_job_lease(&transaction, &self.lease, backend)',
  'SELECT cursor, CAST(source_version AS TEXT)',
  'INSERT INTO index_checkpoints',
  '"rebuild".into()',
  'ON CONFLICT (tenant_id, checkpoint_kind, source_name',
  'cursor = excluded.cursor',
  'COALESCE(excluded.source_version, index_checkpoints.source_version)',
  'COALESCE(excluded.last_delivery_id, index_checkpoints.last_delivery_id)',
  'checkpoint_lease_identity_mismatch',
  'checkpoint_lease_lost',
  'IndexReplayFailure::retryable_static',
  'IndexReplayFailure::permanent_static',
]);
for (const forbidden of ['tokio::spawn', 'DELETE FROM index_checkpoints']) {
  if (postgres.includes(forbidden)) {
    fail(`${postgresPath} contains forbidden scheduler/destructive marker ${forbidden}`);
  }
}

const testsPath = 'crates/rustok-index/src/application/source_replay_tests.rs';
requireMarkers(testsPath, [
  'replay_page_commits_checkpoint_after_mutations',
  'checkpoint_failure_replays_the_same_event_delivery',
  'completed_checkpoint_skips_the_source',
  'checkpoint_watermark_never_regresses',
  'nil_replay_event_is_rejected_before_persistence',
  'vec!["mutation", "checkpoint"]',
  'vec![event_id, event_id]',
]);

requireMarkers('crates/rustok-index/src/application/mod.rs', [
  'mod source_registry;',
  'mod source_replay;',
  'mod source_replay_tests;',
  'IndexSourceCatalog',
  'SharedIndexSourceRegistry',
  'IndexReplayWorker',
  'IndexReplayCheckpointStore',
  'materialize_index_source_registry',
  'register_index_source',
]);
requireMarkers('crates/rustok-index/src/infrastructure/postgres/mod.rs', [
  'mod source_replay;',
  'mod source_replay_job;',
  'PostgresIndexReplayCheckpointStore',
  'PostgresIndexReplayJobStore',
]);
requireMarkers('crates/rustok-index/src/lib.rs', [
  'get_or_insert_with::<IndexSchemaSourceCatalog',
  'get_or_insert_with::<IndexSourceCatalog',
  'PostgresIndexReplayCheckpointStore',
  'PostgresIndexReplayJobStore',
]);
requireMarkers('crates/rustok-index/Cargo.toml', [
  'tracing.workspace = true',
]);
requireMarkers('crates/rustok-index/docs/m5-m6-source-replay-contract.md', [
  'one to 256 unique `EntityKey` values',
  'limit from 1 through 1000',
  'at most 8 KiB',
  '`Retryable` or `Permanent`',
  '`IndexReplayWorker::run_next_page` executes exactly one bounded source page',
  'same non-nil event UUID',
  'source version whenever a page is retried',
  'Commit the next cursor only after every mutation result is durable',
  'existing inbox identity',
  'stable event deliveries idempotent',
  '`PostgresIndexReplayJobStore` owns one exact tenant/source/schema rebuild job',
  '`PostgresIndexReplayCheckpointStore` is constructed from the acquired',
  'it cannot advance the durable cursor',
  'reserved empty values',
  'maintainer-run',
]);
requireMarkers('crates/rustok-index/docs/implementation-plan.md', [
  '- M5/M6 bounded source replay contract: `source_complete_owner_execution_pending`',
  '- M6 one-page replay and durable checkpoint progression: `source_complete`',
  '- M6 replay job leases and checkpoint attempt fencing: `source_complete_owner_execution_pending`',
  '- M7 Product/ProductVariant graph schemas and bounded sources: `source_complete_owner_execution_pending`',
  '- [x] Add a source replay registry with bounded failure classification.',
  '- [x] Add cursor-based `IndexSource::scan` and targeted `load` contracts.',
  '- [x] Add a durable rebuild checkpoint read/write adapter over `index_checkpoints`.',
  '- [x] Add a bounded worker that applies source pages through `PostgresMutationStore` and',
  '- [x] Add durable schema-scoped rebuild jobs, lease/heartbeat, reclaim, attempt fencing,',
  '- [x] Add bounded multi-page execution with heartbeat cadence and immediate pending resume.',
  '- [x] Add durable cancellation requests and fenced between-page terminal cancellation.',
  '- [ ] Add in-page interruption/timeouts, dry-run, and targeted/full/shadow rebuild modes.',
]);
requireMarkers('scripts/verify/verify-index-query-contract.mjs', [
  "'verify-index-source-schema-registry.mjs'",
  "'verify-index-source-replay-contract.mjs'",
  "'verify-index-replay-job-leases.mjs'",
  "'verify-index-query-runtime-composition.mjs'",
]);

console.log('[verify-index-source-replay-contract] OK');

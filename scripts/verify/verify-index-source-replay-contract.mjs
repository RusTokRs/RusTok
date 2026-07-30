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
  'register_index_source(',
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
  'IndexReplayPageStatus::AlreadyComplete',
  '.apply_replay_mutation(',
  'mutation.event_id().to_string()',
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
const applyPosition = worker.indexOf('.apply_replay_mutation(', runStart);
const commitPosition = worker.indexOf('.commit_replay_checkpoint(&checkpoint)', runStart);
if (runStart < 0 || applyPosition < runStart || commitPosition <= applyPosition) {
  fail(`${workerPath} does not commit the checkpoint after mutation application`);
}

const postgresPath = 'crates/rustok-index/src/infrastructure/postgres/source_replay.rs';
const postgres = requireMarkers(postgresPath, [
  'impl IndexReplayMutationSink for PostgresMutationStore',
  'MutationDelivery::from_event(source_name, mutation.clone())',
  'pub struct PostgresIndexReplayCheckpointStore',
  'impl IndexReplayCheckpointStore for PostgresIndexReplayCheckpointStore',
  'SELECT cursor, CAST(source_version AS TEXT)',
  'INSERT INTO index_checkpoints',
  "'rebuild'",
  'ON CONFLICT (tenant_id, checkpoint_kind, source_name',
  'cursor = excluded.cursor',
  'COALESCE(excluded.source_version, index_checkpoints.source_version)',
  'COALESCE(excluded.last_delivery_id, index_checkpoints.last_delivery_id)',
  'IndexReplayFailure::retryable_static',
  'IndexReplayFailure::permanent_static',
]);
for (const forbidden of ['tokio::spawn', 'index_jobs', 'DELETE FROM index_checkpoints']) {
  if (postgres.includes(forbidden)) {
    fail(`${postgresPath} contains forbidden scheduler/destructive marker ${forbidden}`);
  }
}

requireMarkers('crates/rustok-index/src/application/mod.rs', [
  'mod source_registry;',
  'mod source_replay;',
  'IndexSourceCatalog',
  'SharedIndexSourceRegistry',
  'IndexReplayWorker',
  'IndexReplayCheckpointStore',
  'materialize_index_source_registry',
  'register_index_source',
]);
requireMarkers('crates/rustok-index/src/infrastructure/postgres/mod.rs', [
  'mod source_replay;',
  'PostgresIndexReplayCheckpointStore',
]);
requireMarkers('crates/rustok-index/src/lib.rs', [
  'get_or_insert_with::<IndexSchemaSourceCatalog',
  'get_or_insert_with::<IndexSourceCatalog',
  'PostgresIndexReplayCheckpointStore',
]);
requireMarkers('crates/rustok-index/docs/m5-m6-source-replay-contract.md', [
  'one to 256 unique `EntityKey` values',
  'limit from 1 through 1000',
  'at most 8 KiB',
  '`Retryable` or `Permanent`',
  '`IndexReplayWorker::run_next_page` executes exactly one bounded source page',
  'Commit the next cursor only after every mutation result is durable',
  'existing inbox identity makes the same event deliveries idempotent',
  'does not claim a job lease or global worker owner',
  'maintainer-run',
]);
requireMarkers('crates/rustok-index/docs/implementation-plan.md', [
  '- M5/M6 bounded source replay contract: `source_complete_worker_pending`',
  '- [x] Add a source replay registry with bounded failure classification.',
  '- [x] Add cursor-based `IndexSource::scan` and targeted `load` contracts.',
  '- [ ] Add durable jobs, checkpoints, leases, heartbeat, and ownership.',
]);
requireMarkers('scripts/verify/verify-index-query-contract.mjs', [
  "'verify-index-source-schema-registry.mjs'",
  "'verify-index-source-replay-contract.mjs'",
  "'verify-index-query-runtime-composition.mjs'",
]);

console.log('[verify-index-source-replay-contract] OK');

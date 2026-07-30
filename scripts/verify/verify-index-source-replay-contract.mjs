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

requireMarkers('crates/rustok-index/src/application/mod.rs', [
  'mod source_registry;',
  'IndexSourceCatalog',
  'SharedIndexSourceRegistry',
  'materialize_index_source_registry',
  'register_index_source',
]);
requireMarkers('crates/rustok-index/src/lib.rs', [
  'get_or_insert_with::<IndexSchemaSourceCatalog',
  'get_or_insert_with::<IndexSourceCatalog',
]);
requireMarkers('crates/rustok-index/docs/m5-m6-source-replay-contract.md', [
  'one to 256 unique `EntityKey` values',
  'limit from 1 through 1000',
  'at most 8 KiB',
  '`Retryable` or',
  '`Permanent`',
  'does not define that policy yet',
  'maintainer-run',
]);
requireMarkers('crates/rustok-index/docs/implementation-plan.md', [
  '- M5/M6 bounded source replay contract: `source_complete_worker_pending`',
  '- [x] Add a source replay registry with bounded failure classification.',
  '- [x] Add cursor-based `IndexSource::scan` and targeted `load` contracts.',
  '- [ ] Add durable jobs, checkpoints, leases, heartbeat, and ownership.',
  'No durable job runner, checkpoint writer, scheduler, or production replay command',
]);
requireMarkers('scripts/verify/verify-index-query-contract.mjs', [
  "'verify-index-source-schema-registry.mjs'",
  "'verify-index-source-replay-contract.mjs'",
  "'verify-index-query-runtime-composition.mjs'",
]);

console.log('[verify-index-source-replay-contract] OK');
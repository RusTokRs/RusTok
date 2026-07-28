#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const read = (relative) => fs.readFileSync(path.join(root, relative), 'utf8');
const fail = (message) => {
  console.error(`[verify-index-partition-cutover-evidence] ${message}`);
  process.exit(1);
};
const requireMarkers = (relative, markers) => {
  const source = read(relative);
  for (const marker of markers) {
    if (!source.includes(marker)) fail(`${relative} is missing ${marker}`);
  }
  return source;
};

const runner = requireMarkers('ops/benches/src/index_storage/partition_cutover.rs', [
  'INDEX_PARTITION_ALLOW_CUTOVER_EVIDENCE',
  'partition cutover evidence requires PostgreSQL 16',
  'partition cutover evidence requires jit=off',
  'SET enable_partition_pruning = on',
  'SET synchronous_commit = on',
  'manifest.repetitions.cutover',
  'LOCK TABLE {} IN ACCESS EXCLUSIVE MODE',
  'index_pe_cutover_',
  'evidence-only cutover rename choreography',
  'failed to rollback partition cutover rehearsal transaction',
  'partition cutover rehearsal rollback did not restore clone relation identities',
  'canonical or retained snapshot-shadow relations changed during cutover rehearsal',
  'lock_ms',
  'rollback_verified',
  'production_relations_unchanged',
  'fs::hard_link(&temporary, path)',
  'refusing to overwrite {path:?}',
]);

for (const forbidden of [
  'DROP SCHEMA',
  'DROP TABLE',
  'TRUNCATE TABLE',
  'UPDATE index_entities',
  'UPDATE index_links',
  'DELETE FROM index_entities',
  'DELETE FROM index_links',
  'ALTER TABLE "index_entities"',
  'ALTER TABLE "index_links"',
  'COMMIT;',
  'rustok_product',
  'rustok_content',
  'rustok_flex',
]) {
  if (runner.includes(forbidden)) fail(`cutover runner contains forbidden marker ${forbidden}`);
}

requireMarkers('ops/benches/src/bin/index_partition_cutover_evidence.rs', [
  'PartitionCutoverConfig::from_env()',
  'capture_partition_cutover_evidence(&config).await?',
  'index partition cutover evidence complete',
]);
requireMarkers('ops/benches/src/index_storage/mod.rs', [
  'mod partition_cutover;',
  'PartitionCutoverConfig',
  'capture_partition_cutover_evidence',
]);
requireMarkers('ops/benches/Cargo.toml', [
  'name = "index-partition-cutover-evidence"',
  'path = "src/bin/index_partition_cutover_evidence.rs"',
]);
requireMarkers('crates/rustok-index/docs/partition-cutover-evidence.md', [
  'ACCESS EXCLUSIVE',
  'rename choreography only on four empty clones',
  'production and retained snapshot-shadow relations remained unchanged',
  'cutover.json',
]);
requireMarkers('scripts/verify/index-storage-tooling.mjs', [
  "'verify-index-partition-cutover-evidence.mjs'",
]);

console.log('[verify-index-partition-cutover-evidence] OK');
